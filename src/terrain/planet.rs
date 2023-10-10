use std::sync::Arc;
use bevy::asset::Assets;
use bevy::pbr::StandardMaterial;
use bevy::prelude::{Commands, Component, Mesh, Query, ResMut, Transform, With};

use crate::MainCamera;
use crate::terrain::mesh_computer::MeshComputer;
use crate::terrain::terrain_quad_tree::{TerrainQuadTree, TerrainQuadTreeConfig};

#[derive(Component)]
pub(crate) struct Planet {
    terrain_faces: [TerrainQuadTree; 6],
}

#[derive(Clone, Debug)]
pub(crate) enum Face {
    Top,
    Bottom,
    Left,
    Right,
    Front,
    Back
}

impl Planet {
    pub(crate) fn spawn(config: TerrainQuadTreeConfig, commands: &mut Commands, meshes: &mut ResMut<Assets<Mesh>>, materials: &mut ResMut<Assets<StandardMaterial>>, mesh_computer: &mut ResMut<MeshComputer>) {
        let planet = Planet::new(config, commands, meshes, materials, mesh_computer);
        commands.spawn(planet);
    }

    pub(crate) fn new(config: TerrainQuadTreeConfig, commands: &mut Commands, meshes: &mut ResMut<Assets<Mesh>>, materials: &mut ResMut<Assets<StandardMaterial>>, mesh_computer: &mut ResMut<MeshComputer>) -> Self {
        Planet {
            terrain_faces: [
                TerrainQuadTree::new(Face::Top, config, commands, meshes, materials, mesh_computer.clone()),
                TerrainQuadTree::new(Face::Bottom, config, commands, meshes, materials, mesh_computer.clone()),
                TerrainQuadTree::new(Face::Left, config, commands, meshes, materials, mesh_computer.clone()),
                TerrainQuadTree::new(Face::Right, config, commands, meshes, materials, mesh_computer.clone()),
                TerrainQuadTree::new(Face::Front, config, commands, meshes, materials, mesh_computer.clone()),
                TerrainQuadTree::new(Face::Back, config, commands, meshes, materials, mesh_computer.clone())
            ]
        }
    }
}

pub(crate) fn update(
    mut commands: Commands,
    mut query: Query<&mut Planet>,
    camera_query: Query<(&Transform, With<MainCamera>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>
) {
    let camera_transform = camera_query.get_single().expect("No camera found").0;
    for mut planet in query.iter_mut() {
        for terrain_face in planet.terrain_faces.iter_mut() {
            terrain_face.update(&mut commands, &mut meshes, &mut materials, camera_transform.translation);
        }
    }
}
