use std::sync::Mutex;
use bevy::asset::Assets;
use bevy::hierarchy::BuildChildren;

use bevy::math::Vec3;
use bevy::pbr::{PbrBundle, StandardMaterial};
use bevy::prelude::{Color, Commands, Component, default, Entity, Mesh, ResMut, shape, Transform, World};
use bevy::prelude::shape::Icosphere;

use crate::datastructures::quad_tree::{NewRoot, Node, Quadrant, QuadTree, Update};
use crate::terrain::planet::Face;

#[derive(Copy, Clone)]
pub(crate) struct TerrainQuadTreeConfig {
    pub(crate) lod_threshold: f32,
}

pub(crate) struct TerrainQuadTreeNode {
    pub(crate) center: Vec3,
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
        }
    }
}

impl Update<(Vec3, f32, &mut Commands<'_, '_>, &mut ResMut<'_, Assets<Mesh>>, &mut ResMut<'_, Assets<StandardMaterial>>)> for QuadTree<TerrainQuadTreeNode> {
    fn update(&mut self, (camera_position, threshold, commands, meshes, materials): (Vec3, f32, &mut Commands<'_, '_>, &mut ResMut<Assets<Mesh>>, &mut ResMut<Assets<StandardMaterial>>)) {
        let distance = (self.node.center - camera_position).length();
        if distance < threshold {
            self.split();
            self.on_split(commands, meshes, materials);
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
    fn on_split(&mut self, commands: &mut Commands, meshes: &mut ResMut<Assets<Mesh>>, materials: &mut ResMut<Assets<StandardMaterial>>) {
        let mut entity_commands = commands.spawn(PbrBundle {
            mesh: meshes.add(Mesh::try_from(Icosphere { radius: 1.0, subdivisions: 2 }).unwrap()),
            material: materials.add(StandardMaterial {
                base_color: Color::hex("#ffd891").unwrap(),
                ..default()
            }),
            transform: Transform::from_translation(self.node.center),
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
        match face {
            Face::Top => TerrainQuadTreeNode { center: Vec3::new(0.0, 1.0, 0.0), entity: Some(TerrainQuadTreeNode::create_mesh(Vec3::new(0.0, 1.0, 0.0), commands, meshes, materials)) },
            Face::Bottom => TerrainQuadTreeNode { center: Vec3::new(0.0, -1.0, 0.0), entity: Some(TerrainQuadTreeNode::create_mesh(Vec3::new(0.0, -1.0, 0.0), commands, meshes, materials)) },
            Face::Left => TerrainQuadTreeNode { center: Vec3::new(-1.0, 0.0, 0.0), entity: Some(TerrainQuadTreeNode::create_mesh(Vec3::new(-1.0, 0.0, 0.0), commands, meshes, materials)) },
            Face::Right => TerrainQuadTreeNode { center: Vec3::new(1.0, 0.0, 0.0), entity: Some(TerrainQuadTreeNode::create_mesh(Vec3::new(1.0, 0.0, 0.0), commands, meshes, materials)) },
            Face::Front => TerrainQuadTreeNode { center: Vec3::new(0.0, 0.0, 1.0), entity: Some(TerrainQuadTreeNode::create_mesh(Vec3::new(0.0, 0.0, 1.0), commands, meshes, materials)) },
            Face::Back => TerrainQuadTreeNode { center: Vec3::new(0.0, 0.0, -1.0), entity: Some(TerrainQuadTreeNode::create_mesh(Vec3::new(0.0, 0.0, -1.0), commands, meshes, materials)) }
        }
    }

    fn create_mesh(center: Vec3, commands: &mut Commands, meshes: &mut ResMut<Assets<Mesh>>, materials: &mut ResMut<Assets<StandardMaterial>>) -> Entity {
        commands.spawn(PbrBundle {
            mesh: meshes.add(Icosphere { radius: 1.0, subdivisions: 2 }.try_into().unwrap()),
            material: materials.add(StandardMaterial {
                base_color: Color::hex("#ffd891").unwrap(),
                ..default()
            }),
            transform: Transform::from_translation(center),
            ..default()
        }).id()
    }
}

impl Node for TerrainQuadTreeNode {
    fn split_node(&self, quadrant: Quadrant) -> TerrainQuadTreeNode {
        TerrainQuadTreeNode {
            center: match quadrant {
                Quadrant::TopLeft => Vec3::new(self.center.x - 0.5, self.center.y + 0.5, self.center.z),
                Quadrant::TopRight => Vec3::new(self.center.x + 0.5, self.center.y + 0.5, self.center.z),
                Quadrant::BottomLeft => Vec3::new(self.center.x - 0.5, self.center.y - 0.5, self.center.z),
                Quadrant::BottomRight => Vec3::new(self.center.x + 0.5, self.center.y - 0.5, self.center.z),
            },
            entity: None,
        }
    }
}
