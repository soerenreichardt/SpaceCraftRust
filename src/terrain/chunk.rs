use crate::terrain::{Face, CHUNK_SIZE};
use bevy::prelude::*;
use crate::terrain::block::BlockType;
use crate::terrain::terrain_octree::NodeRange;

#[derive(Component)]
pub struct Chunk {
    pub(crate) parent: Option<Entity>,
    /// The planet this chunk belongs to — generation systems resolve the
    /// per-planet `TerrainGenerator` (the planet's noise signature) through
    /// this reference.
    pub(crate) planet: Entity,
    pub(crate) center: Vec3,
    pub(crate) face: Face,
    pub(crate) length: f32,
    pub(crate) scale: f32,
    pub(crate) blocks: Option<Blocks>,
    pub(crate) range: NodeRange,
}

pub struct Blocks {
    blocks: [BlockType; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE],
}

impl Default for Blocks {
    fn default() -> Self {
        Blocks {
            blocks: [BlockType::Air; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE],
        }
    }
}

impl Blocks {
    fn stone() -> Self {
        Blocks {
            blocks: [BlockType::Stone; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE],
        }
    }
}