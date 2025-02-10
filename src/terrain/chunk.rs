use bevy::prelude::*;
use crate::terrain::Face;

#[derive(Component)]
pub struct Chunk {
    pub(crate) parent: Option<Entity>,
    pub(crate) center: Vec3,
    pub(crate) face: Face,
    pub(crate) length: f32,
    pub(crate) scale: f32,
}