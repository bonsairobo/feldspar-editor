use crate::Config;

use feldspar::{
    bb::{
        prelude::{FastArrayCompressionNx2, FromBytesCompression, Lz4},
        storage::database::{
            sled::{self, Tree},
            sled_snapshots::{
                open_snapshot_forest, transactions::create_snapshot_tree,
                TransactionalVersionForest,
            },
            Delta, VersionedChunkDb3,
        },
    },
    prelude::{SdfVoxelMap, ThreadLocalVoxelCache, VoxelDb},
};

use bevy::ecs::prelude::*;
use bevy::input::Input;
use bevy::prelude::KeyCode;
use bevy::tasks::IoTaskPool;

/// Holds persistent metadata about editor state.
pub struct EditorDb {
    tree: Tree,
}

impl EditorDb {
    pub fn new(tree: Tree) -> Self {
        Self { tree }
    }

    pub fn current_version(&self) -> sled::Result<Option<u64>> {
        let version_bytes = self.tree.get(CURRENT_VERSION_KEY)?;
        Ok(version_bytes.map(|b| u64_from_be_slice(&b)))
    }

    pub fn write_current_version(&self, current_version: u64) -> sled::Result<()> {
        self.tree
            .insert(CURRENT_VERSION_KEY, &current_version.to_be_bytes())?;
        Ok(())
    }
}

const CURRENT_VERSION_KEY: &str = "current_version";

fn u64_from_be_slice(s: &[u8]) -> u64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(s);
    u64::from_be_bytes(bytes)
}

pub fn open_voxel_database(mut commands: Commands, config: Res<Config>) {
    let db = sled::Config::default()
        .path(config.database_path.clone())
        .use_compression(false)
        .mode(sled::Mode::LowSpace)
        .open()
        .expect("Failed to open world database");

    let editor_tree = db
        .open_tree("editor")
        .expect("Failed to open saves database");
    let editor_db = EditorDb::new(editor_tree);
    let current_version = editor_db
        .current_version()
        .expect("Failed to read current version");

    let chunks = db
        .open_tree("chunks")
        .expect("Failed to open chunk database");
    let (chunk_versions, chunk_deltas) =
        open_snapshot_forest(&db, "chunks").expect("Failed to open snapshot forest");

    let current_version = current_version.unwrap_or_else(|| {
        chunk_versions
            .transaction(|versions| create_snapshot_tree(TransactionalVersionForest(versions)))
            .expect("Failed to create initial chunk snapshot tree")
    });
    editor_db
        .write_current_version(current_version)
        .expect("Failed to write current version");

    let voxel_db = VoxelDb::new(VersionedChunkDb3::new(
        current_version,
        chunks,
        chunk_versions,
        chunk_deltas,
        FastArrayCompressionNx2::from_bytes_compression(Lz4 { level: 10 }),
    ));

    commands.insert_resource(editor_db);
    commands.insert_resource(voxel_db);
}

pub fn save_map_to_db(
    voxel_db: Res<VoxelDb>,
    local_cache: Res<ThreadLocalVoxelCache>,
    voxel_map: ResMut<SdfVoxelMap>,
    pool: Res<IoTaskPool>,
    keys: Res<Input<KeyCode>>,
) {
    if !keys.just_pressed(KeyCode::S) {
        return;
    }

    log::info!("Saving map to DB");

    let tls = local_cache.get();
    let reader = voxel_map.reader(&tls);

    let deltas: Vec<_> = reader
        .storage()
        .into_iter()
        .map(|(k, v)| Delta::Insert(*k, v))
        .collect();

    log::info!("Writing {} deltas", deltas.len());

    let write_future = voxel_db.chunks().update_current_version(deltas.into_iter());

    for result in pool.scope(|s| s.spawn(write_future)) {
        if result.is_err() {
            panic!("Error saving to DB: {:?}", result);
        }
    }

    futures::executor::block_on(voxel_db.chunks().flush()).expect("Failed to flush chunk DB");
}
