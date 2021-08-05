use super::VoxelFace;
use crate::CursorRay;

use bevy::ecs::prelude::*;
use feldspar::{
    bb::{
        core::{prelude::*, SignedAxis3},
        search::{
            collision::{cast_ray_at_voxels, VoxelRayImpact},
            ncollide3d::query::Ray as NCRay,
        },
    },
    prelude::VoxelBvt,
};

/// The closest voxel that the window cursor is touching.
#[derive(Default)]
pub struct VoxelCursorRayImpact {
    pub maybe_impact: Option<VoxelRayImpact>,
    pub normal: Option<SignedAxis3>,
}

impl VoxelCursorRayImpact {
    pub fn get(&self) -> Option<(&VoxelRayImpact, &SignedAxis3)> {
        match (self.maybe_impact.as_ref(), self.normal.as_ref()) {
            (Some(i), Some(n)) => Some((i, n)),
            _ => None,
        }
    }

    pub fn get_voxel_face(&self) -> Option<VoxelFace> {
        self.get().map(|(impact, normal)| VoxelFace {
            point: impact.point,
            normal: *normal,
        })
    }
}

/// Each frame, a ray is cast at the `VoxelBvt`, and the resulting impact is stored.
pub fn voxel_cursor_impact_system(
    bvt: Res<VoxelBvt>,
    cursor_ray: Res<CursorRay>,
    mut voxel_cursor_impact: ResMut<VoxelCursorRayImpact>,
) {
    voxel_cursor_impact.maybe_impact = None;
    voxel_cursor_impact.normal = None;

    if let CursorRay(Some(ray)) = *cursor_ray {
        if let Some(impact) =
            cast_ray_at_voxels(&*bvt, NCRay::from(ray), std::f32::INFINITY, |_| true)
        {
            let normal = Point3f::from(impact.impact.normal.normalize())
                .round()
                .in_voxel();
            if let Some(normal_axis) = SignedAxis3::from_vector(normal) {
                voxel_cursor_impact.normal = Some(normal_axis);
            }
            voxel_cursor_impact.maybe_impact = Some(impact);
        }
    }
}
