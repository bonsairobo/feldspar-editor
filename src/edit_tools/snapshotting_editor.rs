use super::edit_timeline::EditTimeline;

use feldspar::bb::prelude::*;
use feldspar::prelude::{VoxelEditor, VoxelType};

use bevy::ecs::{prelude::*, system::SystemParam};

#[derive(SystemParam)]
pub struct SnapshottingVoxelEditor<'a> {
    editor: VoxelEditor<'a>,
    timeline: ResMut<'a, EditTimeline>,
}

impl<'a> SnapshottingVoxelEditor<'a> {
    pub fn edit_extent_and_touch_neighbors(
        &mut self,
        extent: Extent3i,
        edit_func: impl FnMut(Point3i, (&mut VoxelType, &mut Sd8)),
    ) {
        self.timeline
            .add_extent_to_current_edit(extent, &self.editor.map.voxels);
        self.editor
            .edit_extent_and_touch_neighbors(extent, edit_func);
    }

    pub fn finish_edit(&mut self) {
        self.timeline.store_current_edit();
    }
}
