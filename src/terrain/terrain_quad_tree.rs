use std::sync::{Arc, Mutex, RwLock};

use bevy::asset::Assets;
use bevy::math::Vec3;
use bevy::pbr::{PbrBundle, StandardMaterial};
use bevy::prelude::{Color, Commands, Component, default, Entity, Mesh, ResMut, Transform, Visibility};
use bevy::prelude::shape::Icosphere;

use crate::datastructures::quad_tree::{NewRoot, Node, Quadrant, QuadTree, Update};
use crate::terrain::mesh_computer::{MeshComputer, MeshRequest};
use crate::terrain::planet::Face;

#[derive(Copy, Clone)]
pub(crate) struct TerrainQuadTreeConfig {
    pub(crate) lod_threshold: f32,
}

#[derive(Debug)]
pub(crate) struct TerrainQuadTreeNode {
    pub(crate) center: Vec3,
    face: Face,
    pub(crate) entity: Option<Entity>,
    mesh_computer: MeshComputer,
}

#[derive(Component)]
pub(crate) struct TerrainQuadTree {
    quad_tree: Mutex<QuadTree<TerrainQuadTreeNode>>,
    face: Face,
    config: TerrainQuadTreeConfig,
}

impl TerrainQuadTree {
    pub(crate) fn new(face: Face, config: TerrainQuadTreeConfig, commands: &mut Commands, meshes: &mut ResMut<Assets<Mesh>>, materials: &mut ResMut<Assets<StandardMaterial>>, mesh_computer: MeshComputer) -> Self {
        TerrainQuadTree {
            quad_tree: Mutex::new(QuadTree::new_root((face.clone(), commands, meshes, materials, mesh_computer))),
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

impl NewRoot<(Face, &mut Commands<'_, '_>, &mut ResMut<'_, Assets<Mesh>>, &mut ResMut<'_, Assets<StandardMaterial>>, MeshComputer)> for QuadTree<TerrainQuadTreeNode> {
    fn new_root((face, commands, meshes, materials, mesh_computer): (Face, &mut Commands, &mut ResMut<Assets<Mesh>>, &mut ResMut<Assets<StandardMaterial>>, MeshComputer)) -> Self {
        QuadTree {
            parent: None,
            children: None,
            node: TerrainQuadTreeNode::new_root(&face, commands, meshes, materials, mesh_computer),
            max_depth: 2,
            level: 0,
        }
    }
}

impl Update<(Vec3, f32, &mut Commands<'_, '_>, &mut ResMut<'_, Assets<Mesh>>, &mut ResMut<'_, Assets<StandardMaterial>>)> for QuadTree<TerrainQuadTreeNode> {
    fn update(&mut self, (camera_position, threshold, commands, meshes, materials): (Vec3, f32, &mut Commands<'_, '_>, &mut ResMut<Assets<Mesh>>, &mut ResMut<Assets<StandardMaterial>>)) {
        let distance = (self.node.center - camera_position).length() * (2.0f32).powf(-(self.level as f32));
        if distance < threshold {
            if self.split() {
                for child in self.children.clone().unwrap() {
                    self.schedule_mesh_creation(child.clone());
                }
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
    fn schedule_mesh_creation(&mut self, child: Arc<RwLock<QuadTree<TerrainQuadTreeNode>>>) {
        self.node.mesh_computer.schedule(MeshRequest { node: child });
    }
}

impl TerrainQuadTreeNode {
    fn new_root(face: &Face, commands: &mut Commands, meshes: &mut ResMut<Assets<Mesh>>, materials: &mut ResMut<Assets<StandardMaterial>>, mesh_computer: MeshComputer) -> Self {
        let center = match face {
            Face::Top => Vec3::new(0.0, 0.5, 0.0),
            Face::Bottom => Vec3::new(0.0, -0.5, 0.0),
            Face::Left => Vec3::new(-0.5, 0.0, 0.0),
            Face::Right => Vec3::new(0.5, 0.0, 0.0),
            Face::Front => Vec3::new(0.0, 0.0, 0.5),
            Face::Back => Vec3::new(0.0, 0.0, -0.5)
        };

        TerrainQuadTreeNode {
            center,
            face: face.clone(),
            entity: Some(TerrainQuadTreeNode::create_mesh(center, commands, meshes, materials)),
            mesh_computer
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

    fn compute_center_for_face(face: &Face, (offset1, offset2): (f64, f64)) -> Vec3 {
        match face {
            Face::Top | Face::Bottom => Vec3::new(offset1 as f32, 0.0, offset2 as f32),
            Face::Left | Face::Right => Vec3::new(0.0, offset1 as f32, offset2 as f32),
            Face::Front | Face::Back => Vec3::new(offset1 as f32, offset2 as f32, 0.0)
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
            center: TerrainQuadTreeNode::compute_center_for_face(&self.face, offsets),
            face: self.face.clone(),
            entity: None,
            mesh_computer: self.mesh_computer.clone()
        }
    }
}
