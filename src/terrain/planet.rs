use crate::camera::MainCamera;
use crate::terrain::mesh_generator::MeshGenerator2;
use crate::terrain::terrain_quad_tree::TerrainQuadTree;
use bevy::prelude::*;
use crate::terrain::Face;

type PlanetSide = TerrainQuadTree;

#[derive(Component)]
pub(crate) struct Planet {
    planet_sides: [PlanetSide; 6],
}

impl Planet {
    pub(crate) fn new(size: u8, scale: f32, commands: &mut Commands, mesh_generator: &mut MeshGenerator2, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        Planet {
            planet_sides: [
                PlanetSide::new(size, scale, Face::Back, commands, mesh_generator, meshes, materials),
                PlanetSide::new(size, scale, Face::Front, commands, mesh_generator, meshes, materials),
                PlanetSide::new(size, scale, Face::Left, commands, mesh_generator, meshes, materials),
                PlanetSide::new(size, scale, Face::Right, commands, mesh_generator, meshes, materials),
                PlanetSide::new(size, scale, Face::Top, commands, mesh_generator, meshes, materials),
                PlanetSide::new(size, scale, Face::Bottom, commands, mesh_generator, meshes, materials),
            ]
        }
    }
}

#[derive(Event)]
pub struct ChunkCreateEvent {
    entity: Entity,
}

pub fn update_lod(
    mut planet_query: Query<&mut Planet>,
    camera_query: Query<&Transform, With<MainCamera>>,
    mut commands: Commands,
    mut mesh_generator: ResMut<MeshGenerator2>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>
) {
    let camera_transform = camera_query.single();
    let camera_translation = camera_transform.translation;

    for mut planet in planet_query.iter_mut() {
        for mut planet_side in planet.planet_sides.iter_mut() {
            planet_side.update(camera_translation, &mut commands, mesh_generator.as_mut(), &mut meshes, &mut materials)
        }
    }
}
