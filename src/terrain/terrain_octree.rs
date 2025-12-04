use crate::terrain::chunk::{Blocks, Chunk};
use crate::terrain::octree::{Octant, Octree, Splittable};
use crate::terrain::Face;
use bevy::prelude::*;

pub(crate) struct TerrainOctree {
    octree: Octree<TerrainOctreeNode>
}

pub(crate) struct TerrainOctreeNode {
    center: Vec3,
    face: Face,
    length: f32,
    scale: f32,
    entity: Entity,
}

impl Splittable for TerrainOctreeNode {
    fn split(&self, octant: Octant, _level: u8, commands: &mut Commands) -> Self {
        let offset = octant.center_on_unit_cube();
        let length = self.length / 2.0;
        let relative_center = offset * length;
        let center = self.center + relative_center;

        let entity = commands.spawn((
            Chunk {
                parent: Some(self.entity),
                center,
                face: self.face.clone(),
                length,
                scale: self.scale,
                blocks: Blocks::default(),
            },
            Transform::from_translation(relative_center),
        )).id();

        commands.entity(self.entity).add_child(entity);

        TerrainOctreeNode {
            center,
            face: self.face.clone(),
            length,
            scale: self.scale,
            entity,
        }
    }
}

#[derive(Event)]
pub struct RemoveTerrainChildren(pub Entity);

impl Octree<TerrainOctreeNode> {
    pub fn update(&mut self, camera_translation: Vec3, commands: &mut Commands, event_writer: &mut EventWriter<RemoveTerrainChildren>) {
        let center_on_sphere = self.node.center.normalize() * self.node.scale;
        let distance_to_camera = (camera_translation - center_on_sphere).length();
        let threshold = 2.0f32.powf(-(self.level as f32)) * 7.0 * self.node.scale;

        if let Some(children) = self.children() {
            if distance_to_camera > threshold {
                self.merge();
                event_writer.write(RemoveTerrainChildren(self.node.entity));
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
                    let node = &mut child.node;
                    entity_commands.add_child(node.entity);
                }
            }
        }
    }
}

impl TerrainOctree {
    pub fn new(depth: u8, scale: f32, face: Face, commands: &mut Commands) -> Self {
        let length = 2.0 * scale;
        let center = face.direction_vector() * scale;
        let entity_commands = commands.spawn((
            Chunk {
                parent: None,
                center,
                face: face.clone(),
                length,
                scale,
                blocks: Blocks::default(),
            },
            Transform::from_translation(center),
        ));
        let root = TerrainOctreeNode {
            center,
            face,
            length,
            scale,
            entity: entity_commands.id(),
        };
        TerrainOctree {
            octree: Octree::new(depth, 0, root)
        }
    }

    #[inline(always)]
    pub fn update(&mut self, camera_translation: Vec3, commands: &mut Commands, event_writer: &mut EventWriter<RemoveTerrainChildren>) {
        self.octree.update(camera_translation, commands, event_writer);
    }
}