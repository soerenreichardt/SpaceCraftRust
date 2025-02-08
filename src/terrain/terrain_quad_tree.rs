use crate::terrain::mesh_generator::MeshGenerator2;
use crate::terrain::quad_tree::{QuadTree, Quadrant, Splittable};
use bevy::prelude::*;
use crate::terrain::Face;

pub(crate) struct TerrainQuadTree {
    quad_tree: QuadTree<TerrainQuadTreeNode>
}

pub(crate) struct TerrainQuadTreeNode {
    pub(crate) center: Vec3,
    pub(crate) face: Face,
    pub(crate) length: f32,
    pub(crate) scale: f32,
    pub(crate) entity: Entity,
}

impl Splittable for TerrainQuadTreeNode {
    fn split(&self, quadrant: Quadrant, _level: u8, commands: &mut Commands) -> Self {
        let offsets: (f64, f64) = match quadrant {
            Quadrant::TopLeft => (-0.5, 0.5),
            Quadrant::TopRight => (0.5, 0.5),
            Quadrant::BottomLeft => (-0.5, -0.5),
            Quadrant::BottomRight => (0.5, -0.5)
        };
        let length = self.length / 2.0;
        let offsets = (offsets.0 * length as f64, offsets.1 * length as f64);
        let center = Self::compute_center_for_face(&self.face, self.center, offsets);
        let scaled_center = self.face.direction_vector() + center;
        let entity = commands.spawn_empty().id();
        TerrainQuadTreeNode {
            center: scaled_center, 
            face: self.face.clone(), 
            length,
            scale: self.scale,
            entity
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

impl QuadTree<TerrainQuadTreeNode> {
    pub fn update(&mut self, camera_translation: Vec3, commands: &mut Commands, mesh_generator: &mut MeshGenerator2, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) {
        let distance_to_camera = (camera_translation - self.node.center * self.node.scale).length();
        let threshold = 2.0f32.powf(-(self.level as f32)) * 7.0 * self.node.scale;

        if let Some(children) = self.children() {
            if distance_to_camera > threshold {
                self.merge();
                commands.entity(self.node.entity).despawn_descendants();
                return;
            }

            for child in children {
                child.update(camera_translation, commands, mesh_generator, meshes, materials);
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
                    let pbr_bundle = mesh_generator.generate_mesh(node.center, node.length, node.face.clone(), node.scale, meshes, materials);
                    entity_commands
                        .add_child(node.entity)
                        .commands()
                        .entity(node.entity)
                        .insert(pbr_bundle);
                }
            }
        }
    }
}

impl TerrainQuadTree {
    pub fn new(depth: u8, scale: f32, face: Face, commands: &mut Commands, mesh_generator: &mut MeshGenerator2, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        let mut entity_commands = commands.spawn_empty();
        let root = TerrainQuadTreeNode {
            center: face.direction_vector(),
            face,
            length: 2.0,
            scale,
            entity: entity_commands.id()
        };
        let pbr_bundle = mesh_generator.generate_mesh(root.center, root.length, root.face.clone(), root.scale, meshes, materials);
        entity_commands.insert(pbr_bundle);
        TerrainQuadTree {
            quad_tree: QuadTree::new(depth, 0, root)
        }
    }

    pub fn update(&mut self, camera_translation: Vec3, commands: &mut Commands, mesh_generator: &mut MeshGenerator2, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) {
        self.quad_tree.update(camera_translation, commands, mesh_generator, meshes, materials);
    }
}
