use crate::camera::MainCamera;
use crate::terrain::terrain_quad_tree::TerrainQuadTree;
use crate::terrain::Face;
use bevy::prelude::*;

type PlanetSide = TerrainQuadTree;

#[derive(Component)]
pub(crate) struct Planet {
    planet_sides: [PlanetSide; 6],
}

impl Planet {
    pub(crate) fn new(size: u8, scale: f32, commands: &mut Commands) -> Self {
        Planet {
            planet_sides: [
                PlanetSide::new(size, scale, Face::Back, commands),
                PlanetSide::new(size, scale, Face::Front, commands),
                PlanetSide::new(size, scale, Face::Left, commands),
                PlanetSide::new(size, scale, Face::Right, commands),
                PlanetSide::new(size, scale, Face::Top, commands),
                PlanetSide::new(size, scale, Face::Bottom, commands),
            ]
        }
    }
}

#[derive(Event)]
pub struct ChunkCreateEvent {
    entity: Entity,
}

pub fn update_lod(mut planet_query: Query<&mut Planet>, camera_query: Query<&Transform, With<MainCamera>>, mut commands: Commands) {
    let camera_transform = camera_query.single();
    let camera_translation = camera_transform.translation;

    for mut planet in planet_query.iter_mut() {
        for mut planet_side in planet.planet_sides.iter_mut() {
            planet_side.update(camera_translation, &mut commands)
        }
    }
}
