use crate::camera::MainCamera;
use crate::terrain::Face;
use bevy::prelude::*;
use crate::terrain::generate::TerrainGenerator;
use crate::terrain::terrain_octree::{RemoveTerrainChildren, TerrainOctree};

type PlanetSide = TerrainOctree;

#[derive(Component)]
pub(crate) struct Planet {
    planet_sides: [PlanetSide; 6],
    pub(crate) size: u8,
    pub(crate) terrain_generator: TerrainGenerator
}

impl Planet {
    pub(crate) fn new(size: u8, scale: f32, planet_entity: Entity, commands: &mut Commands) -> Self {
        Planet {
            planet_sides: [
                PlanetSide::new(size, scale, Face::Back, planet_entity, commands),
                PlanetSide::new(size, scale, Face::Front, planet_entity, commands),
                PlanetSide::new(size, scale, Face::Left, planet_entity, commands),
                PlanetSide::new(size, scale, Face::Right, planet_entity, commands),
                PlanetSide::new(size, scale, Face::Top, planet_entity, commands),
                PlanetSide::new(size, scale, Face::Bottom, planet_entity, commands),
            ],
            size,
            terrain_generator: TerrainGenerator::new(scale, 0)
        }
    }
}

pub fn update_lod(mut planet_query: Query<&mut Planet>, camera_query: Query<&Transform, With<MainCamera>>, mut commands: Commands, mut event_writer: EventWriter<RemoveTerrainChildren>) {
    let camera_transform = camera_query.single().unwrap();
    let camera_translation = camera_transform.translation;

    for mut planet in planet_query.iter_mut() {
        for planet_side in planet.planet_sides.iter_mut() {
            planet_side.update(camera_translation, &mut commands, &mut event_writer)
        }
    }
}
