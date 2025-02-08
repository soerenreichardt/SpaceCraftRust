use bevy::prelude::Vec3;

pub(crate) mod terrain_quad_tree;
pub(crate) mod planet;
pub(crate) mod mesh_generator;
mod quad_tree;

#[derive(Clone, Debug)]
pub(crate) enum Face {
    Top,
    Bottom,
    Left,
    Right,
    Front,
    Back,
}

impl Face {
    pub(crate) fn direction_vector(&self) -> Vec3 {
        match self {
            Face::Top => Vec3::Y,
            Face::Bottom => -Vec3::Y,
            Face::Left => -Vec3::X,
            Face::Right => Vec3::X,
            Face::Front => -Vec3::Z,
            Face::Back => Vec3::Z,
        }
    }

    pub(crate) fn perpendicular_vectors(&self) -> (Vec3, Vec3) {
        match self {
            Face::Top => (Vec3::X, Vec3::Z),
            Face::Bottom => (Vec3::X, -Vec3::Z),
            Face::Left => (Vec3::Y, Vec3::Z),
            Face::Right => (Vec3::Y, -Vec3::Z),
            Face::Front => (Vec3::X, Vec3::Y),
            Face::Back => (Vec3::X, -Vec3::Y),
        }
    }
}