use crate::camera::MainCamera;
use crate::terrain::Face;
use bevy::prelude::*;
use crate::terrain::generate::TerrainGenerator;
use crate::terrain::terrain_octree::{RemoveTerrainChildren, TerrainOctree};

type PlanetSide = TerrainOctree;

#[derive(Component)]
pub(crate) struct Planet {
    planet_sides: [PlanetSide; 6],
    /// This planet's terrain signature: seeded fbm, sea level, amplitude.
    /// Per-planet (not a resource) — every planet generates differently.
    pub(crate) terrain_generator: TerrainGenerator,
}

impl Planet {
    /// Builds a planet's six face octrees. `planet` must be an already
    /// spawned (but still empty) entity — every chunk is stamped with it so
    /// generation systems can resolve `terrain_generator`; the caller
    /// inserts the returned `Planet` component onto the entity afterwards.
    pub(crate) fn new(planet: Entity, size: u8, scale: f32, seed: u32, commands: &mut Commands) -> Self {
        Planet {
            planet_sides: [
                PlanetSide::new(size, scale, Face::Back, planet, commands),
                PlanetSide::new(size, scale, Face::Front, planet, commands),
                PlanetSide::new(size, scale, Face::Left, planet, commands),
                PlanetSide::new(size, scale, Face::Right, planet, commands),
                PlanetSide::new(size, scale, Face::Top, planet, commands),
                PlanetSide::new(size, scale, Face::Bottom, planet, commands),
            ],
            terrain_generator: TerrainGenerator::new(scale, seed),
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
