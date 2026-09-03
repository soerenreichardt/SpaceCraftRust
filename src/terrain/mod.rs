use bevy::prelude::Vec3;

pub(crate) mod terrain_octree;
pub(crate) mod planet;
mod octree;
mod chunk;
mod block;
pub(crate) mod mesh;
mod generate;

pub const CHUNK_SIZE: usize = 32;

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
            Face::Front => Vec3::Z,
            Face::Back => -Vec3::Z,
        }
    }

    pub(crate) fn perpendicular_vectors(&self) -> (Vec3, Vec3) {
        match self {
            Face::Top => (-Vec3::Z, Vec3::X),
            Face::Bottom => (Vec3::Z, -Vec3::X),
            Face::Left => (Vec3::Z, Vec3::Y),
            Face::Right => (-Vec3::Z, Vec3::Y),
            Face::Front => (Vec3::X, Vec3::Y),
            Face::Back => (-Vec3::X, Vec3::Y),
        }
    }
}