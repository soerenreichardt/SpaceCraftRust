use crate::terrain::{Face, CHUNK_SIZE};
use bevy::prelude::*;
use crate::terrain::block::BlockType;
use crate::terrain::terrain_octree::NodeRange;

#[derive(Component, Clone)]
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

#[derive(Clone)]
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
    pub(crate) fn stone() -> Self {
        Blocks {
            blocks: [BlockType::Stone; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE],
        }
    }

    pub(crate) fn from_blocks(blocks: [BlockType; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE]) -> Self {
        Blocks { blocks }
    }

    #[inline]
    fn index(i: usize, j: usize, k: usize) -> usize {
        i + CHUNK_SIZE * (j + CHUNK_SIZE * k)
    }

    pub(crate) fn get(&self, i: usize, j: usize, k: usize) -> BlockType {
        self.blocks[Self::index(i, j, k)]
    }

    pub(crate) fn is_solid(&self, i: usize, j: usize, k: usize) -> bool {
        self.get(i, j, k) != BlockType::Air
    }
}