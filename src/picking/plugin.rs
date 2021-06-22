use crate::EditorState;

use super::{
    ray_impact::voxel_cursor_impact_system,
    voxel_cursor::{voxel_clicking_system, VoxelCursorStates},
    VoxelCursorRayImpact,
};

use bevy::{app::prelude::*, ecs::prelude::*};

/// Manages the `VoxelCursorRayImpact` and `VoxelCursorStates` resources.
pub struct VoxelPickingPlugin;

impl Plugin for VoxelPickingPlugin {
    fn build(&self, app: &mut AppBuilder) {
        app.insert_resource(VoxelCursorRayImpact::default())
            .insert_resource(VoxelCursorStates::default())
            .add_system_set(
                SystemSet::on_update(EditorState::Editing)
                    .with_system(voxel_cursor_impact_system.system())
                    .with_system(voxel_clicking_system.system()),
            );
    }
}
