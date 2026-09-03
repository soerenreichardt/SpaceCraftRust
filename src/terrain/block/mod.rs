#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BlockType {
    Air,
    Dirt,
    Grass,
    Stone,
    Water,
    Sand,
    Snow,
    Bedrock,
}
impl BlockType {
    /// Vertex color as linear RGBA. Values are picked in sRGB (the
    /// intuitive space) and converted — writing raw sRGB numbers into the
    /// linear attribute made the terrain neon-bright. Grass shades the
    /// surface tops, Stone the subsurface, cliffs, and cave interiors;
    /// tune the sRGB triples to taste.
    pub(crate) fn color(&self) -> [f32; 4] {
        let (r, g, b) = match self {
            BlockType::Air => (0.0, 0.0, 0.0),
            BlockType::Dirt => (0.45, 0.33, 0.20),
            BlockType::Grass => (0.33, 0.55, 0.22),
            BlockType::Stone => (0.55, 0.55, 0.58),
            BlockType::Water => (0.20, 0.42, 0.75),
            BlockType::Sand => (0.82, 0.76, 0.58),
            BlockType::Snow => (0.92, 0.94, 0.97),
            BlockType::Bedrock => (0.28, 0.28, 0.32),
        };
        let linear: bevy::prelude::LinearRgba = bevy::prelude::Color::srgb(r, g, b).into();
        [linear.red, linear.green, linear.blue, linear.alpha]
    }
}
