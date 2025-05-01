use crate::camera::MainCamera;
use crate::terrain::terrain_quad_tree::{RemoveTerrainChildren, TerrainQuadTree};
use crate::terrain::Face;
use bevy::prelude::*;
use noise::BasicMulti;

type PlanetSide = TerrainQuadTree;

#[derive(Component)]
pub(crate) struct Planet {
    planet_sides: [PlanetSide; 6],
}

impl Planet {
    pub(crate) fn new(size: u8, scale: f32, commands: &mut Commands) -> Self {
        let noise = BasicMulti::new(0);
        Planet {
            planet_sides: [
                PlanetSide::new(size, scale, Face::Back, commands, &noise),
                PlanetSide::new(size, scale, Face::Front, commands, &noise),
                PlanetSide::new(size, scale, Face::Left, commands, &noise),
                PlanetSide::new(size, scale, Face::Right, commands, &noise),
                PlanetSide::new(size, scale, Face::Top, commands, &noise),
                PlanetSide::new(size, scale, Face::Bottom, commands, &noise),
            ]
        }
    }
}

pub fn update_lod(mut planet_query: Query<&mut Planet>, camera_query: Query<&Transform, With<MainCamera>>, mut commands: Commands, mut event_writer: EventWriter<RemoveTerrainChildren>) {
    let camera_transform = camera_query.single();
    let camera_translation = camera_transform.translation;

    for mut planet in planet_query.iter_mut() {
        for mut planet_side in planet.planet_sides.iter_mut() {
            planet_side.update(camera_translation, &mut commands, &mut event_writer)
        }
    }
}
