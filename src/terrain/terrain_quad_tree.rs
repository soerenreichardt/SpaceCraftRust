use crate::terrain::chunk::{Chunk, ChunkInfo};
use crate::terrain::quad_tree::{QuadTree, Quadrant, Splittable};
use crate::terrain::Face;
use bevy::prelude::*;
use noise::{BasicMulti, Perlin};

pub(crate) struct TerrainQuadTree {
    quad_tree: QuadTree<TerrainQuadTreeNode>
}

pub(crate) struct TerrainQuadTreeNode {
    pub(crate) chunk_info: ChunkInfo,
    pub(crate) entity: Entity,
    pub(crate) block_level: u8
}

impl Splittable for TerrainQuadTreeNode {
    fn split(&self, quadrant: Quadrant, level: u8, commands: &mut Commands) -> Self {
        let offsets: (f64, f64) = match quadrant {
            Quadrant::TopLeft => (-0.5, 0.5),
            Quadrant::TopRight => (0.5, 0.5),
            Quadrant::BottomLeft => (-0.5, -0.5),
            Quadrant::BottomRight => (0.5, -0.5)
        };
        let length = self.chunk_info.length / 2.0;
        let offsets = (offsets.0 * length as f64, offsets.1 * length as f64);
        let face = &self.chunk_info.face;
        let center = Self::compute_center_for_face(face, self.chunk_info.center, offsets);
        let scaled_center = face.direction_vector() + center;
        
        let chunk_info = ChunkInfo {
            length,
            center: scaled_center,
            face: face.clone(),
            scale: self.chunk_info.scale,
            noise: self.chunk_info.noise.clone()
        };
        
        let chunk = Chunk::generate(false, Some(self.entity), chunk_info.clone());
        let entity = commands.spawn(chunk).id();

        TerrainQuadTreeNode {
            chunk_info,
            entity,
            block_level: self.block_level
        }
    }
}

impl TerrainQuadTreeNode {
    fn compute_center_for_face(face: &Face, center: Vec3, (offset1, offset2): (f64, f64)) -> Vec3 {
        match face {
            Face::Top | Face::Bottom => Vec3::new(center.x + offset1 as f32, 0.0, center.z + offset2 as f32),
            Face::Left | Face::Right => Vec3::new(0.0, center.y + offset1 as f32, center.z + offset2 as f32),
            Face::Front | Face::Back => Vec3::new(center.x + offset1 as f32, center.y + offset2 as f32, 0.0)
        }
    }
}

#[derive(Event)]
pub struct RemoveTerrainChildren(pub Entity);

impl QuadTree<TerrainQuadTreeNode> {
    pub fn update(&mut self, camera_translation: Vec3, commands: &mut Commands, event_writer: &mut EventWriter<RemoveTerrainChildren>) {
        // TODO: normalize center on center computation
        let distance_to_camera = (camera_translation - self.node.chunk_info.center.normalize() * self.node.chunk_info.scale).length();
        let threshold = 2.0f32.powf(-(self.level as f32)) * 7.0 * self.node.chunk_info.scale;

        if let Some(children) = self.children() {
            if distance_to_camera > threshold {
                self.merge();
                event_writer.send(RemoveTerrainChildren(self.node.entity));
                return;
            }

            for child in children {
                child.update(camera_translation, commands, event_writer);
            }
        } else {
            if distance_to_camera <= threshold {
                if !self.split(commands) {
                    return;
                }
                let children = self.children.as_mut().unwrap();
                let mut entity_commands = commands.entity(self.node.entity);
                for child in children.iter_mut() {
                    let mut node = &mut child.node;
                    entity_commands.add_child(node.entity);
                }
            }
        }
    }
}

impl TerrainQuadTree {
    pub fn new(depth: u8, scale: f32, face: Face, commands: &mut Commands, noise: &BasicMulti<Perlin>,) -> Self {
        let chunk_info = ChunkInfo {
            center: face.clone().direction_vector(),
            face: face.clone(),
            length: 2.0,
            scale,
            noise: noise.clone(),
        };
        let chunk = Chunk::generate(false, None, chunk_info.clone());
        let mut entity_commands = commands.spawn(chunk);
        let root = TerrainQuadTreeNode {
            chunk_info,
            entity: entity_commands.id(),
            block_level: depth
        };
        TerrainQuadTree {
            quad_tree: QuadTree::new(depth, 0, root)
        }
    }

    pub fn update(&mut self, camera_translation: Vec3, commands: &mut Commands, event_writer: &mut EventWriter<RemoveTerrainChildren>) {
        self.quad_tree.update(camera_translation, commands, event_writer);
    }
}
