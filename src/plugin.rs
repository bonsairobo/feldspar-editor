use crate::{
    create_camera_entity, BevyConfig, CameraConfig, CameraPlugin, Config, CursorPositionPlugin,
    EditToolsPlugin, ImmediateModePlugin, VoxelPickingPlugin,
};

use feldspar::{
    bb::core::prelude::*,
    bb::storage::{prelude::*, sled},
    SdfVoxelMap, ThreadLocalVoxelCache, VoxelEditor, VoxelRenderAssets, VoxelType, VoxelWorldDb,
    VoxelWorldPlugin,
};

use bevy::{
    app::{prelude::*, PluginGroupBuilder},
    asset::{prelude::*, AssetPlugin},
    core::CorePlugin,
    ecs::prelude::*,
    input::{Input, InputPlugin},
    math::prelude::*,
    pbr::{Light, LightBundle, PbrPlugin},
    prelude::KeyCode,
    render::{prelude::*, wireframe::WireframeConfig, wireframe::WireframePlugin, RenderPlugin},
    tasks::IoTaskPool,
    transform::{components::Transform, TransformPlugin},
    wgpu::{WgpuFeature, WgpuFeatures, WgpuOptions, WgpuPlugin},
    window::{WindowDescriptor, WindowPlugin},
    winit::WinitPlugin,
};

/// The first-party plugins that we need from Bevy.
struct BevyPlugins {
    config: BevyConfig,
}

impl BevyPlugins {
    fn new(config: BevyConfig) -> Self {
        Self { config }
    }
}

impl PluginGroup for BevyPlugins {
    fn build(&mut self, group: &mut PluginGroupBuilder) {
        group.add(CorePlugin::default());
        group.add(TransformPlugin::default());
        group.add(InputPlugin::default());
        group.add(WindowPlugin::default());
        group.add(AssetPlugin::default());
        group.add(RenderPlugin::default());
        group.add(PbrPlugin::default());
        group.add(WinitPlugin::default());
        group.add(WgpuPlugin::default());

        if self.config.wireframes {
            group.add(WireframePlugin::default());
        }
    }
}

pub struct EditorPlugin {
    config: Config,
}

impl EditorPlugin {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut AppBuilder) {
        app
            // Bevy stuff.
            .insert_resource(WindowDescriptor {
                width: 1600.0,
                height: 900.0,
                title: "Feldspar Editor".to_string(),
                ..Default::default()
            })
            .insert_resource(ClearColor(Color::rgb(0.2, 0.2, 0.4)))
            .insert_resource(Msaa { samples: 4 })
            .insert_resource(WgpuOptions {
                features: WgpuFeatures {
                    // The Wireframe plugin requires NonFillPolygonMode feature
                    features: vec![WgpuFeature::NonFillPolygonMode],
                },
                ..Default::default()
            })
            .insert_resource(WireframeConfig {
                global: self.config.bevy.wireframes,
            })
            .add_plugins(BevyPlugins::new(self.config.bevy))
            // Editor stuff.
            .insert_resource(self.config)
            .add_plugin(VoxelWorldPlugin::new(
                self.config.feldspar,
                EditorState::Editing,
            ))
            .add_plugin(CursorPositionPlugin)
            .add_plugin(ImmediateModePlugin)
            .add_plugin(CameraPlugin)
            .add_plugin(VoxelPickingPlugin)
            .add_plugin(EditToolsPlugin::new(self.config.feldspar.map.chunk_shape))
            .add_state(EditorState::Loading)
            // Load assets.
            .add_system_set(
                SystemSet::on_enter(EditorState::Loading)
                    .with_system(start_loading_render_assets.system()),
            )
            .add_system_set(
                SystemSet::on_update(EditorState::Loading)
                    .with_system(wait_for_assets_loaded.system()),
            )
            // Initialize entities.
            .add_system_set(
                SystemSet::on_enter(EditorState::Editing)
                    // HACK: we MUST load chunks on entering this state so they will be seen as dirty by the mesh generator
                    .with_system(load_chunks_from_db.system().label("load_chunks"))
                    .with_system(initialize_editor.system().after("load_chunks")),
            )
            // Save the map to our database
            // TODO: this should happen in veldspar proper as edits are made
            .add_system_set(
                SystemSet::on_update(EditorState::Editing).with_system(save_map_to_db.system()),
            );
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum EditorState {
    Loading,
    Editing,
}

// TODO: we should just spawn our camera as a "witness" and have feldspar load the map around it
fn load_chunks_from_db(
    mut commands: Commands,
    mut voxel_editor: VoxelEditor,
    pool: Res<IoTaskPool>,
) {
    let db = sled::Config::default()
        .path("/tmp/world1".to_owned())
        .use_compression(false)
        .mode(sled::Mode::LowSpace)
        .open()
        .expect("Failed to open world database");
    let chunk_tree = db
        .open_tree("chunks")
        .expect("Failed to open chunk database");
    let world_db = VoxelWorldDb::new(chunk_tree);

    let load_extent = Extent3i::from_min_and_shape(Point3i::fill(-1024), Point3i::fill(2048));
    let load_future = world_db.load_chunks_into_map(0, load_extent, &mut voxel_editor);
    pool.scope(|s| s.spawn(load_future));

    commands.insert_resource(world_db);
}

struct LoadingTexture(Handle<Texture>);

fn start_loading_render_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(LoadingTexture(
        asset_server.load("grass_rock_snow_dirt/base_color.png"),
    ));
}

fn wait_for_assets_loaded(
    mut commands: Commands,
    loading_texture: Res<LoadingTexture>,
    textures: Res<Assets<Texture>>,
    mut state: ResMut<State<EditorState>>,
) {
    if textures.get(&loading_texture.0).is_some() {
        log::info!("Done loading mesh texture");

        commands.insert_resource(VoxelRenderAssets {
            mesh_base_color: loading_texture.0.clone(),
        });
        state.set(EditorState::Editing).unwrap();
    }
}

fn initialize_editor(mut commands: Commands, mut voxel_editor: VoxelEditor, config: Res<Config>) {
    if voxel_editor.edit_buffer_is_empty() {
        // TODO: remove this once we can create voxels out of thin air
        log::info!("Initializing voxels");
        let write_extent = Extent3i::from_min_and_shape(PointN([0, 0, 0]), PointN([64, 64, 64]));
        voxel_editor.edit_extent_and_touch_neighbors(write_extent, |_p, (voxel_type, dist)| {
            *voxel_type = VoxelType(2);
            *dist = Sd8::from(-10.0);
        });
    }

    create_lights(&mut commands);
    initialize_camera(&mut commands, config.camera);
}

fn create_lights(commands: &mut Commands) {
    for p in [
        Vec3::new(-100.0, 100.0, -100.0),
        Vec3::new(-100.0, 100.0, 100.0),
        Vec3::new(100.0, 100.0, -100.0),
        Vec3::new(100.0, 100.0, 100.0),
    ]
    .iter()
    {
        commands.spawn_bundle(LightBundle {
            transform: Transform::from_translation(*p),
            light: Light {
                intensity: 40000.0,
                range: 800.0,
                ..Default::default()
            },
            ..Default::default()
        });
    }
}

fn initialize_camera(commands: &mut Commands, camera_config: CameraConfig) {
    let eye = Vec3::new(100.0, 100.0, 100.0);
    let target = Vec3::new(0.0, 0.0, 0.0);
    create_camera_entity(commands, camera_config, eye, target);
}

fn save_map_to_db(
    world_db: Res<VoxelWorldDb>,
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

    let chunk_refs: Vec<_> = reader.storage().into_iter().map(|(k, v)| (*k, v)).collect();

    let write_future = world_db.chunks().write_chunks(chunk_refs.into_iter());

    for result in pool.scope(|s| s.spawn(write_future)) {
        if result.is_err() {
            panic!("Error saving to DB: {:?}", result);
        }
    }

    world_db
        .chunks()
        .tree()
        .flush()
        .expect("Failed to flush DB");
}
