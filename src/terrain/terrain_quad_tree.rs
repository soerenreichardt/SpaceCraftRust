use std::sync::{Arc, RwLock, Weak};

use bevy::asset::Assets;
use bevy::ecs::system::EntityCommands;
use bevy::hierarchy::{BuildChildren, DespawnRecursiveExt};
use bevy::math::Vec3;
use bevy::pbr::{PbrBundle, StandardMaterial};
use bevy::prelude::{Bundle, Color, Commands, Component, ComputedVisibility, default, Entity, Mesh, Query, ResMut, Transform, Visibility, VisibilityBundle, With};
use bevy::prelude::shape::Icosphere;

use crate::camera::MainCamera;
use crate::terrain::planet::Face;

pub(crate) enum Quadrant {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

pub(crate) struct TerrainQuadTree {
    pub(crate) parent: Option<usize>,
    pub(crate) children: Option<[TerrainQuadTreeChild; 4]>,
    pub(crate) node: TerrainQuadTreeNode,
    pub(crate) max_depth: u8,
    pub(crate) level: u8,
}

pub(crate) struct TerrainQuadTreeChild(pub(crate) Arc<RwLock<TerrainQuadTree>>);

pub(crate) struct TerrainQuadTreeNode {
    pub(crate) center: Vec3,
    face: Face,
}

#[derive(Component)]
pub(crate) struct TerrainQuadTreeComponent(Weak<RwLock<TerrainQuadTree>>);

impl TerrainQuadTree {
    pub(crate) fn root(face: Face, max_depth: u8) -> Self {
        let center = match face {
            Face::Top => Vec3::new(0.0, 0.5, 0.0),
            Face::Bottom => Vec3::new(0.0, -0.5, 0.0),
            Face::Left => Vec3::new(-0.5, 0.0, 0.0),
            Face::Right => Vec3::new(0.5, 0.0, 0.0),
            Face::Front => Vec3::new(0.0, 0.0, 0.5),
            Face::Back => Vec3::new(0.0, 0.0, -0.5)
        };
        TerrainQuadTree {
            parent: None,
            children: None,
            node: TerrainQuadTreeNode {
                center,
                face
            },
            max_depth,
            level: 0,
        }
    }

    pub(crate) fn new(parent: *const TerrainQuadTree, node: TerrainQuadTreeNode, max_depth: u8, level: u8) -> Self {
        TerrainQuadTree {
            parent: Some(parent as usize),
            children: None,
            node,
            max_depth,
            level,
        }
    }

    pub(crate) fn split(&mut self, mut entity_commands: EntityCommands, meshes: &mut ResMut<Assets<Mesh>>, materials: &mut ResMut<Assets<StandardMaterial>>) {
        if self.children.is_none() && self.level < self.max_depth {
            self.children = Some([
                self.new_child(Quadrant::TopLeft, &mut entity_commands, meshes, materials),
                self.new_child(Quadrant::TopRight, &mut entity_commands, meshes, materials),
                self.new_child(Quadrant::BottomLeft, &mut entity_commands, meshes, materials),
                self.new_child(Quadrant::BottomRight, &mut entity_commands, meshes, materials),
            ]);
        }
    }

    pub(crate) fn merge(&mut self) {
        self.children.take();
    }

    pub(crate) fn parent(&self) -> Option<&TerrainQuadTree> {
        unsafe {
            match self.parent {
                Some(parent) => Some((parent as *const TerrainQuadTree).as_ref().expect("Parent could not be dereferenced")),
                None => None
            }
        }
    }

    fn new_child(&self, quadrant: Quadrant, entity_commands: &mut EntityCommands, meshes: &mut ResMut<Assets<Mesh>>, materials: &mut ResMut<Assets<StandardMaterial>>) -> TerrainQuadTreeChild {
        let node = self.node.split(quadrant, self.level + 1);
        let center = node.center.clone();
        let child: TerrainQuadTreeChild = TerrainQuadTree::new(
            self as *const TerrainQuadTree,
            node,
            self.max_depth,
            self.level + 1
        ).into();

        let render_component = Self::compute_mesh(center, meshes, materials);
        let rendered_terrain_bundle = RenderedTerrainBundle { terrain_component: (&child).into(), render_component };
        let child_component = entity_commands.commands().spawn(rendered_terrain_bundle).id();
        entity_commands.add_child(child_component);
        child
    }

    pub(crate) fn compute_mesh(center: Vec3, mut meshes: &mut ResMut<Assets<Mesh>>, mut materials: &mut ResMut<Assets<StandardMaterial>>) -> PbrBundle {
        PbrBundle {
            mesh: meshes.add(Mesh::try_from(Icosphere { radius: 0.1, subdivisions: 2 }).unwrap()),
            material: materials.add(StandardMaterial {
                base_color: Color::hex("#ffd891").unwrap(),
                ..default()
            }),
            transform: Transform::from_translation(center),
            visibility: Visibility::Visible,
            ..default()
        }
    }
}

#[derive(Bundle)]
pub(crate) struct RenderedTerrainBundle {
    pub(crate) terrain_component: TerrainQuadTreeComponent,
    pub(crate) render_component: PbrBundle,
}

impl Into<TerrainQuadTreeChild> for TerrainQuadTree {
    fn into(self) -> TerrainQuadTreeChild {
        TerrainQuadTreeChild(Arc::new(RwLock::new(self)))
    }
}

impl Into<TerrainQuadTreeComponent> for &TerrainQuadTreeChild {
    fn into(self) -> TerrainQuadTreeComponent {
        TerrainQuadTreeComponent(Arc::downgrade(&self.0))
    }
}

impl TerrainQuadTreeNode {
    fn split(&self, quadrant: Quadrant, level: u8) -> TerrainQuadTreeNode {
        let offsets: (f64, f64) = match quadrant {
            Quadrant::TopLeft => (-0.5, 0.5),
            Quadrant::TopRight => (0.5, 0.5),
            Quadrant::BottomLeft => (-0.5, -0.5),
            Quadrant::BottomRight => (0.5, -0.5)
        };
        let offsets = (offsets.0 / (2.0f64.powf(level as f64)), offsets.1 / 2.0f64.powf(level as f64));
        TerrainQuadTreeNode {
            center: Self::compute_center_for_face(&self.face, offsets),
            face: self.face.clone()
        }
    }

    fn compute_center_for_face(face: &Face, (offset1, offset2): (f64, f64)) -> Vec3 {
        match face {
            Face::Top | Face::Bottom => Vec3::new(offset1 as f32, 0.0, offset2 as f32),
            Face::Left | Face::Right => Vec3::new(0.0, offset1 as f32, offset2 as f32),
            Face::Front | Face::Back => Vec3::new(offset1 as f32, offset2 as f32, 0.0)
        }
    }
}

pub(crate) fn update(
    mut commands: Commands,
    quad_tree_query: Query<(Entity, &TerrainQuadTreeComponent)>,
    camera_query: Query<(&Transform, With<MainCamera>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let camera_transform = camera_query.get_single().expect("No camera found").0;
    for (entity, quad_tree) in quad_tree_query.iter() {
        match quad_tree.0.upgrade() {
            Some(quad_tree_lock) => {
                let (center, level) = {
                    let quad_tree = quad_tree_lock.read().unwrap();
                    (quad_tree.node.center, quad_tree.level)
                };
                let distance_to_camera = (camera_transform.translation - center).length();
                let threshold = 2.0f32.powf(-(level as f32));
                let mut quad_tree = quad_tree_lock.write().unwrap();
                let mut entity_commands = commands.entity(entity);
                if distance_to_camera < threshold {
                    entity_commands.insert(VisibilityBundle { visibility: Visibility::Hidden, computed: ComputedVisibility::default() });
                    quad_tree.split(entity_commands, &mut meshes, &mut materials);
                } else {
                    entity_commands.insert(VisibilityBundle { visibility: Visibility::Visible, computed: ComputedVisibility::default() });
                    quad_tree.merge()
                }
            }
            None => {
                commands.get_entity(entity).map(|entity| entity.despawn_recursive());
            }
        }
    }
}