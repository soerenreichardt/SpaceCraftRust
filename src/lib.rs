use bevy::app::{App, Plugin, Startup, Update};
use bevy::math::Vec3;
use bevy::pbr::DirectionalLight;
use bevy::prelude::{Camera3d, Commands, PerspectiveProjection, Projection, Transform};
use bevy::utils::default;

use crate::camera::MainCamera;
use crate::terrain::planet::Planet;
use crate::terrain::terrain_octree::RemoveTerrainChildren;
use crate::terrain::{Face, CHUNK_SIZE};
use crate::terrain::mesh_generator;

pub(crate) mod terrain;
pub(crate) mod camera;

pub struct SpaceCraftPlugin;

impl Plugin for SpaceCraftPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(Startup, SpaceCraftPlugin::setup)
            .add_systems(Update, camera::update_camera_position)
            .add_systems(Update, terrain::planet::update_lod)
            .add_systems(Update, mesh_generator::generate_chunk_meshes)
            .add_systems(Update, mesh_generator::restore_visibility)
            .add_event::<RemoveTerrainChildren>();
    }
}

impl SpaceCraftPlugin {
    fn setup(mut commands: Commands) {
        let radius = 3;
        let scale = 2.0f32.powf(radius as f32 - 1.0) * CHUNK_SIZE as f32;
        commands.spawn((
            Camera3d { ..default() },
            Transform::from_translation(Face::Back.direction_vector() * scale + 20.0).looking_at(Vec3::default(), Vec3::Y),
            Projection::Perspective(PerspectiveProjection { ..default() }.into()),
            MainCamera
        ));

        let planet = Planet::new(radius, scale, &mut commands);
        commands.spawn(planet);
        commands.spawn((
            DirectionalLight { ..default() },
            Transform::from_translation(Vec3::new(15.0, 15.0, 15.0)),
        ));
    }
}
