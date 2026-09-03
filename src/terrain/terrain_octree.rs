use crate::terrain::chunk::Chunk;
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
    planet_entity: Entity,
    range: NodeRange,
}

#[derive(Clone)]
pub(crate) struct NodeRange {
    x: DimensionRange,
    y: DimensionRange,
    z: DimensionRange,
}

#[derive(Clone)]
pub(crate) struct DimensionRange(pub(crate) u32, pub(crate) u32);

impl Splittable for TerrainOctreeNode {
    fn split(&self, octant: Octant, _level: u8, commands: &mut Commands) -> Self {
        let offset = octant.center_on_unit_cube();
        let length = self.length / 2.0;
        let relative_center = offset * length;
        let center = self.center + relative_center;
        let range = self.range.split(octant);

        let entity = commands.spawn((
            Chunk {
                parent: Some(self.entity),
                center,
                face: self.face.clone(),
                length,
                scale: self.scale,
                planet_entity: self.planet_entity,
                blocks: None,
                range: range.clone(),
            },
            Transform::from_translation(relative_center),
            Visibility::default(),
        )).id();

        commands.entity(self.entity).add_child(entity);

        TerrainOctreeNode {
            center,
            face: self.face.clone(),
            length,
            scale: self.scale,
            entity,
            planet_entity: self.planet_entity,
            range,
        }
    }
}

#[derive(Event)]
pub struct RemoveTerrainChildren(pub Entity);

impl Octree<TerrainOctreeNode> {
    pub fn update(&mut self, camera_translation: Vec3, commands: &mut Commands, event_writer: &mut EventWriter<RemoveTerrainChildren>) {
        let center_on_sphere = self.node.center.normalize() * self.node.scale;
        let distance_to_camera = (camera_translation - center_on_sphere).length();
        let threshold = 2.0f32.powf(-(self.level as f32)) * 5.0 * self.node.scale;

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
    pub fn new(depth: u8, scale: f32, face: Face, planet_entity: Entity, commands: &mut Commands) -> Self {
        let length = 2.0 * scale;
        let center = face.direction_vector() * scale;
        let range = NodeRange::from_scale(scale);
        let entity_commands = commands.spawn((
            Chunk {
                parent: None,
                center,
                face: face.clone(),
                length,
                scale,
                planet_entity,
                blocks: None,
                range: range.clone(),
            },
            Transform::from_translation(center),
            Visibility::default(),
        ));
        let root = TerrainOctreeNode {
            center,
            face,
            length,
            scale,
            entity: entity_commands.id(),
            planet_entity,
            range,
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


impl NodeRange {
    pub(crate) fn x(&self, face: Face) -> &DimensionRange {
        match face {
            Face::Left | Face::Right => &self.z,
            Face::Top | Face::Bottom => &self.x,
            Face::Front | Face::Back => &self.x,
        }
    }

    pub(crate) fn y(&self, face: Face) -> &DimensionRange {
        match face {
            Face::Left | Face::Right => &self.y,
            Face::Top | Face::Bottom => &self.z,
            Face::Front | Face::Back => &self.y,
        }
    }

    pub(crate) fn z(&self, face: Face) -> &DimensionRange {
        match face {
            Face::Left | Face::Right => &self.x,
            Face::Top | Face::Bottom => &self.y,
            Face::Front | Face::Back => &self.z,
        }
    }

    pub(crate) fn from_scale(scale: f32) -> Self {
        let max = scale as u32;
        NodeRange {
            x: DimensionRange(0, max),
            y: DimensionRange(0, max),
            z: DimensionRange(0, max),
        }
    }

    pub(crate) fn split(&self, octant: Octant) -> Self {
        match octant {
            Octant::TopLeftFront => NodeRange {
                x: self.x.lower_half(),
                y: self.y.upper_half(),
                z: self.z.upper_half(),
            },
            Octant::TopLeftBack => NodeRange {
                x: self.x.lower_half(),
                y: self.y.upper_half(),
                z: self.z.lower_half(),
            },
            Octant::TopRightFront => NodeRange {
                x: self.x.upper_half(),
                y: self.y.upper_half(),
                z: self.z.upper_half(),
            },
            Octant::TopRightBack => NodeRange {
                x: self.x.upper_half(),
                y: self.y.upper_half(),
                z: self.z.lower_half(),
            },
            Octant::BottomLeftFront => NodeRange {
                x: self.x.lower_half(),
                y: self.y.lower_half(),
                z: self.z.upper_half(),
            },
            Octant::BottomLeftBack => NodeRange {
                x: self.x.lower_half(),
                y: self.y.lower_half(),
                z: self.z.lower_half(),
            },
            Octant::BottomRightFront => NodeRange {
                x: self.x.upper_half(),
                y: self.y.lower_half(),
                z: self.z.upper_half(),
            },
            Octant::BottomRightBack => NodeRange {
                x: self.x.upper_half(),
                y: self.y.lower_half(),
                z: self.z.lower_half(),
            },
        }
    }
}

impl DimensionRange {
    fn mid(&self) -> u32 {
        (self.0 + self.1) / 2
    }

    fn upper_half(&self) -> Self {
        DimensionRange(self.mid(), self.1)
    }

    fn lower_half(&self) -> Self {
        DimensionRange(self.0, self.mid())
    }
}