use std::sync::{Arc, RwLock, Weak};

use bevy::ecs::system::EntityCommands;
use bevy::hierarchy::{BuildChildren, DespawnRecursiveExt};
use bevy::math::Vec3;
use bevy::prelude::{Commands, Component, Entity, Query, ResMut, Transform, Visibility, VisibilityBundle, With};

use crate::camera::MainCamera;
use crate::terrain::mesh_generator::{MeshGenerator, Request};
use crate::terrain::planet::Face;

#[derive(PartialEq, Clone)]
pub(crate) enum Quadrant {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

pub(crate) struct TerrainQuadTree {
    pub(crate) parent: Option<usize>,
    pub(crate) children: Option<[TerrainQuadTreeChild; 4]>,
    pub(crate) node: Arc<RwLock<TerrainQuadTreeNode>>,
    pub(crate) max_depth: u8,
    pub(crate) level: u8,
    pub(crate) scale: f32
}

#[derive(Debug)]
pub(crate) struct TerrainQuadTreeNode {
    pub(crate) center: Vec3,
    pub(crate) face: Face,
    pub(crate) entity: Option<Entity>,
    pub(crate) length: f32,
}

pub(crate) struct TerrainQuadTreeChild(pub(crate) Arc<RwLock<TerrainQuadTree>>);

#[derive(Component)]
pub(crate) struct TerrainQuadTreeComponent(Weak<RwLock<TerrainQuadTree>>);

impl TerrainQuadTree {
    pub(crate) fn root(face: Face, max_depth: u8, scale: f32) -> Self {
        let center = face.direction_vector() * scale;
        TerrainQuadTree {
            parent: None,
            children: None,
            node: Arc::new(RwLock::new(TerrainQuadTreeNode {
                center,
                face,
                entity: None,
                length: 2.0 * scale,
            })),
            max_depth,
            level: 0,
            scale
        }
    }

    pub(crate) fn new(parent: *const TerrainQuadTree, node: Arc<RwLock<TerrainQuadTreeNode>>, max_depth: u8, level: u8, scale: f32) -> Self {
        TerrainQuadTree {
            parent: Some(parent as usize),
            children: None,
            node,
            max_depth,
            level,
            scale
        }
    }

    pub(crate) fn split(&mut self, entity_commands: &mut EntityCommands, mesh_generator: &mut ResMut<MeshGenerator>) -> bool {
        if self.children.is_none() && self.level < self.max_depth {
            self.children = Some([
                self.new_child(Quadrant::TopLeft, entity_commands, mesh_generator),
                self.new_child(Quadrant::TopRight, entity_commands, mesh_generator),
                self.new_child(Quadrant::BottomLeft, entity_commands, mesh_generator),
                self.new_child(Quadrant::BottomRight, entity_commands, mesh_generator),
            ]);
            return true;
        }
        false
    }

    pub(crate) fn merge(&mut self) -> bool {
        self.children.take().is_some()
    }

    pub(crate) fn parent(&self) -> Option<&TerrainQuadTree> {
        unsafe {
            match self.parent {
                Some(parent) => Some((parent as *const TerrainQuadTree).as_ref().expect("Parent could not be dereferenced")),
                None => None
            }
        }
    }

    fn new_child(&self, quadrant: Quadrant, entity_commands: &mut EntityCommands, mesh_generator: &mut ResMut<MeshGenerator>) -> TerrainQuadTreeChild {
        let node = Arc::new(RwLock::new(self.node.write().unwrap().split(quadrant.clone(), self.scale)));
        let child: TerrainQuadTreeChild = TerrainQuadTree::new(
            self as *const TerrainQuadTree,
            node.clone(),
            self.max_depth,
            self.level + 1,
            self.scale
        ).into();

        let child_component = entity_commands.commands().spawn::<TerrainQuadTreeComponent>((&child).into()).id();
        node.write().unwrap().entity = Some(child_component);
        entity_commands.add_child(child_component);

        mesh_generator.queue_generate_mesh_request(Request::create(node.clone(), self.scale));

        child
    }

}

impl From<TerrainQuadTree> for TerrainQuadTreeChild {
    fn from(val: TerrainQuadTree) -> Self {
        TerrainQuadTreeChild(Arc::new(RwLock::new(val)))
    }
}

impl From<&TerrainQuadTreeChild> for TerrainQuadTreeComponent {
    fn from(val: &TerrainQuadTreeChild) -> Self {
        TerrainQuadTreeComponent(Arc::downgrade(&val.0))
    }
}

impl TerrainQuadTreeNode {
    fn split(&self, quadrant: Quadrant, scale: f32) -> TerrainQuadTreeNode {
        let offsets: (f64, f64) = match quadrant {
            Quadrant::TopLeft => (-0.5, 0.5),
            Quadrant::TopRight => (0.5, 0.5),
            Quadrant::BottomLeft => (-0.5, -0.5),
            Quadrant::BottomRight => (0.5, -0.5)
        };
        let length = self.length / 2.0;
        let offsets = (offsets.0 * length as f64, offsets.1 * length as f64);
        let center = Self::compute_center_for_face(&self.face, self.center, offsets);
        let scaled_center = self.face.direction_vector() * scale + center;
        TerrainQuadTreeNode {
            center: scaled_center,
            face: self.face.clone(),
            entity: None,
            length
        }
    }

    fn compute_center_for_face(face: &Face, center: Vec3, (offset1, offset2): (f64, f64)) -> Vec3 {
        match face {
            Face::Top | Face::Bottom => Vec3::new(center.x + offset1 as f32, 0.0, center.z + offset2 as f32),
            Face::Left | Face::Right => Vec3::new(0.0, center.y + offset1 as f32, center.z + offset2 as f32),
            Face::Front | Face::Back => Vec3::new(center.x + offset1 as f32, center.y + offset2 as f32, 0.0)
        }
    }
}

pub(crate) fn update(
    mut commands: Commands,
    quad_tree_query: Query<(Entity, &TerrainQuadTreeComponent)>,
    camera_query: Query<&Transform, With<MainCamera>>,
    mut mesh_generator: ResMut<MeshGenerator>,
) {
    let camera_transform = camera_query.get_single().expect("No camera found");
    for (entity, quad_tree) in quad_tree_query.iter() {
        match quad_tree.0.upgrade() {
            Some(quad_tree_lock) => {
                let (center, level, scale) = {
                    let quad_tree = quad_tree_lock.read().unwrap();
                    (quad_tree.node.clone().read().unwrap().center, quad_tree.level, quad_tree.scale)
                };
                let distance_to_camera = (camera_transform.translation - center).length();
                let threshold = 2.0f32.powf(-(level as f32)) * 10.0 * scale;
                let mut quad_tree = quad_tree_lock.write().unwrap();
                let mut entity_commands = commands.entity(entity);
                if distance_to_camera < threshold {
                    if quad_tree.split(&mut entity_commands, &mut mesh_generator) {
                        entity_commands.insert( VisibilityBundle { visibility: Visibility::Hidden, ..Default::default() });
                    }
                } else {
                    if quad_tree.merge() {
                        entity_commands.insert(VisibilityBundle { visibility: Visibility::Visible, ..Default::default() });
                    }
                }
            }
            None => {
                if let Some(entity) = commands.get_entity(entity) {
                    entity.despawn_recursive();
                }
            }
        }
    }
}