use crate::terrain::{Face, CHUNK_SIZE};
use bevy::prelude::*;
use crate::terrain::block::BlockType;
use crate::terrain::terrain_octree::NodeRange;

#[derive(Component)]
pub struct Chunk {
    pub(crate) parent: Option<Entity>,
    pub(crate) center_on_cube: Vec3,
    pub(crate) center_on_sphere: Vec3,
    pub(crate) face: Face,
    pub(crate) length: f32,
    pub(crate) scale: f32,
    pub(crate) planet_entity: Entity,
    pub(crate) blocks: Option<Blocks>,
    pub(crate) range: NodeRange,
    pub(crate) level: u8,
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
    pub fn set(&mut self, block: BlockType, x: usize, y: usize, z: usize) {
        self.blocks[z * CHUNK_SIZE * CHUNK_SIZE + y * CHUNK_SIZE + x] = block;
    }

    // Not consumed by the mesh path yet; used by the fill_blocks tests and
    // future block queries.
    #[allow(dead_code)]
    pub fn get(&self, x: usize, y: usize, z: usize) -> BlockType {
        self.blocks[z * CHUNK_SIZE * CHUNK_SIZE + y * CHUNK_SIZE + x]
    }
}