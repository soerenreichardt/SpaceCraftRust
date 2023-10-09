use std::sync::Mutex;

use bevy::asset::Assets;
use bevy::hierarchy::BuildChildren;
use bevy::math::Vec3;
use bevy::pbr::{PbrBundle, StandardMaterial};
use bevy::prelude::{Color, Commands, Component, default, Entity, Mesh, ResMut, Transform, Visibility};
use bevy::prelude::shape::Icosphere;

use crate::datastructures::quad_tree::{NewRoot, Node, Quadrant, QuadTree, Update};
use crate::terrain::planet::Face;

#[derive(Copy, Clone)]
pub(crate) struct TerrainQuadTreeConfig {
    pub(crate) lod_threshold: f32,
}

pub(crate) struct TerrainQuadTreeNode {
    pub(crate) center: Vec3,
    face: Face,
    entity: Option<Entity>,
}

#[derive(Component)]
pub(crate) struct TerrainQuadTree {
    quad_tree: Mutex<QuadTree<TerrainQuadTreeNode>>,
    face: Face,
    config: TerrainQuadTreeConfig,
}

impl TerrainQuadTree {
    pub(crate) fn new(face: Face, config: TerrainQuadTreeConfig, commands: &mut Commands, meshes: &mut ResMut<Assets<Mesh>>, materials: &mut ResMut<Assets<StandardMaterial>>) -> Self {
        TerrainQuadTree {
            quad_tree: Mutex::new(QuadTree::new_root((face.clone(), commands, meshes, materials))),
            face,
            config,
        }
    }

    pub(crate) fn update(
        &mut self,
        commands: &mut Commands<'_, '_>,
        meshes: &mut ResMut<Assets<Mesh>>,
        materials: &mut ResMut<Assets<StandardMaterial>>,
        camera_position: Vec3,
    ) {
        self.quad_tree.get_mut().expect("Could not acquire write lock").update((camera_position, self.config.lod_threshold, commands, meshes, materials));
    }
}

impl NewRoot<(Face, &mut Commands<'_, '_>, &mut ResMut<'_, Assets<Mesh>>, &mut ResMut<'_, Assets<StandardMaterial>>)> for QuadTree<TerrainQuadTreeNode> {
    fn new_root((face, commands, meshes, materials): (Face, &mut Commands<'_, '_>, &mut ResMut<'_, Assets<Mesh>>, &mut ResMut<'_, Assets<StandardMaterial>>)) -> Self {
        QuadTree {
            parent: None,
            children: None,
            node: TerrainQuadTreeNode::new(&face, commands, meshes, materials),
            max_depth: 3,
            level: 0
        }
    }
}

impl Update<(Vec3, f32, &mut Commands<'_, '_>, &mut ResMut<'_, Assets<Mesh>>, &mut ResMut<'_, Assets<StandardMaterial>>)> for QuadTree<TerrainQuadTreeNode> {
    fn update(&mut self, (camera_position, threshold, commands, meshes, materials): (Vec3, f32, &mut Commands<'_, '_>, &mut ResMut<Assets<Mesh>>, &mut ResMut<Assets<StandardMaterial>>)) {
        let distance = (self.node.center - camera_position).length();
        if distance < threshold {
            if self.split() {
                for child in self.children.clone().unwrap() {
                    child.write().unwrap().create_mesh_entity(commands, meshes, materials)
                }
                commands.entity(self.node.entity.unwrap());
            }
        } else if distance > threshold {
            self.merge();
        }

        if let Some(children) = &mut self.children {
            for child in children.iter_mut() {
                child.write().expect("Could not acquire write lock").update((camera_position, threshold, commands, meshes, materials));
            }
        }
    }
}

impl QuadTree<TerrainQuadTreeNode> {
    fn create_mesh_entity(&mut self, commands: &mut Commands, meshes: &mut ResMut<Assets<Mesh>>, materials: &mut ResMut<Assets<StandardMaterial>>) {
        let mut entity_commands = commands.spawn(PbrBundle {
            mesh: meshes.add(Mesh::try_from(Icosphere { radius: 0.1, subdivisions: 2 }).unwrap()),
            material: materials.add(StandardMaterial {
                base_color: Color::hex("#ffd891").unwrap(),
                ..default()
            }),
            transform: Transform::from_translation(self.node.center),
            visibility: Visibility::Visible,
            ..default()
        });

        if let Some(parent) = self.parent() {
            entity_commands.set_parent(parent.node.entity.expect("No parent entity found"));
        }

        self.node.entity = Some(entity_commands.id());
    }
}

impl TerrainQuadTreeNode {
    fn new(face: &Face, commands: &mut Commands, meshes: &mut ResMut<Assets<Mesh>>, materials: &mut ResMut<Assets<StandardMaterial>>) -> Self {
        let center = match face {
            Face::Top => Vec3::new(0.0, 1.0, 0.0),
            Face::Bottom => Vec3::new(0.0, -1.0, 0.0),
            Face::Left => Vec3::new(-1.0, 0.0, 0.0),
            Face::Right => Vec3::new(1.0, 0.0, 0.0),
            Face::Front => Vec3::new(0.0, 0.0, 1.0),
            Face::Back => Vec3::new(0.0, 0.0, -1.0)
        };

        TerrainQuadTreeNode {
            center,
            face: face.clone(),
            entity: Some(TerrainQuadTreeNode::create_mesh(center, commands, meshes, materials)),
        }
    }

    fn create_mesh(center: Vec3, commands: &mut Commands, meshes: &mut ResMut<Assets<Mesh>>, materials: &mut ResMut<Assets<StandardMaterial>>) -> Entity {
        commands.spawn(PbrBundle {
            mesh: meshes.add(Icosphere { radius: 0.1, subdivisions: 2 }.try_into().unwrap()),
            material: materials.add(StandardMaterial {
                base_color: Color::hex("#ffd891").unwrap(),
                ..default()
            }),
            transform: Transform::from_translation(center),
            visibility: Visibility::Visible,
            ..default()
        }).id()
    }

    fn compute_center_for_face(old_center: Vec3, face: &Face, (offset1, offset2): (f64, f64)) -> Vec3 {
        match face {
            Face::Top | Face::Bottom => Vec3::new(old_center.x + offset1 as f32, 0.0, old_center.z + offset2 as f32),
            Face::Left | Face::Right => Vec3::new(0.0, old_center.y + offset1 as f32, old_center.z + offset2 as f32),
            Face::Front | Face::Back => Vec3::new(old_center.x + offset1 as f32, old_center.y + offset2 as f32, 0.0)
        }
    }
}

impl Node for TerrainQuadTreeNode {
    fn split_node(&self, quadrant: Quadrant, level: u8) -> TerrainQuadTreeNode {
        let offsets: (f64, f64) = match quadrant {
            Quadrant::TopLeft => (-0.5, 0.5),
            Quadrant::TopRight => (0.5, 0.5),
            Quadrant::BottomLeft => (-0.5, -0.5),
            Quadrant::BottomRight => (0.5, -0.5)
        };
        let offsets = (offsets.0 / (2.0f64.powf(level as f64)), offsets.1 / 2.0f64.powf(level as f64));
        TerrainQuadTreeNode {
            center: TerrainQuadTreeNode::compute_center_for_face(self.center, &self.face, offsets),
            face: self.face.clone(),
            entity: None,
        }
    }
}
