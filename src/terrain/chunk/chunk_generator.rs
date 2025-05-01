use noise::{BasicMulti, Perlin};
use crate::terrain::chunk::{Chunk, ChunkInfo};

struct ChunkGenerator {
    planet_size: u8,
    noise: BasicMulti<Perlin>,
}

impl ChunkGenerator {
    pub fn generate(&self, level: u8, chunk_info: &ChunkInfo) -> Chunk {
        todo!()
    }
}