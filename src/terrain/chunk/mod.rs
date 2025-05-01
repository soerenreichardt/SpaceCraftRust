use crate::terrain::block::BlockType;
use crate::terrain::mesh_generator::{compute_chunk_plane, MESH_SIZE};
use crate::terrain::Face;
use bevy::prelude::*;
use noise::{BasicMulti, Perlin};

pub mod chunk_generator;

#[derive(Component)]
pub enum Chunk {
    Smooth(SmoothChunk),
    Block(BlockChunk),
}

pub struct SmoothChunk {
    pub(crate) parent: Option<Entity>,
    pub(crate) chunk_info: ChunkInfo,
}

pub struct BlockChunk {
    parent: Entity,
    chunk_info: ChunkInfo,
    chunk_plane: ChunkPlane,
    sub_chunks: Vec<SubChunk>,
}

#[derive(Clone)]
pub struct ChunkInfo {
    pub(crate) center: Vec3,
    pub(crate) face: Face,
    pub(crate) length: f32,
    pub(crate) scale: f32,
    pub(crate) noise: BasicMulti<Perlin>,
}

pub struct ChunkPlane {
    vertices: Vec<Vec3>,
}

struct SubChunk {
    level: u8,
    blocks: [BlockType; MESH_SIZE * MESH_SIZE * MESH_SIZE]
}

impl Chunk {
    pub(crate) fn generate(block_level: bool, parent: Option<Entity>, chunk_info: ChunkInfo) -> Self {
        if block_level {
            Chunk::Block(BlockChunk::generate(parent.expect("Block level should have a parent"), chunk_info))
        } else {
            Chunk::Smooth(SmoothChunk::generate(parent, chunk_info))
        }
    }

    pub(crate) fn parent(&self) -> Option<Entity> {
        match self {
            Chunk::Smooth(smooth_chunk) => smooth_chunk.parent,
            Chunk::Block(block_chunk) => Some(block_chunk.parent)
        }
    }
}

impl SmoothChunk {
    pub fn generate(parent: Option<Entity>, chunk_info: ChunkInfo) -> Self {
        SmoothChunk {
            parent,
            chunk_info
        }
    }
}

impl BlockChunk {
    pub fn generate(parent: Entity, chunk_info: ChunkInfo) -> Self {
        let chunk_plane = compute_chunk_plane(chunk_info);
        todo!()
    }
}
