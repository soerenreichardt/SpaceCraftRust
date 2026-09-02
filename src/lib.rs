use bevy::app::{App, Plugin, Startup, Update};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::math::Vec3;
use bevy::pbr::DirectionalLight;
use bevy::prelude::{Camera3d, Commands, PerspectiveProjection, Projection, Transform};
use bevy::utils::default;

use crate::camera::MainCamera;
use crate::terrain::planet::Planet;
use crate::terrain::terrain_octree::RemoveTerrainChildren;
use crate::terrain::{Face, CHUNK_SIZE};
use terrain::mesh::mesh_generator;
use terrain::mesh::mesh_generator::MeshGenerator;

pub(crate) mod terrain;
pub(crate) mod camera;

pub struct SpaceCraftPlugin;

impl Plugin for SpaceCraftPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(Startup, SpaceCraftPlugin::setup)
            .add_systems(Update, (
                camera::update_camera_position,
                terrain::planet::update_lod,
                mesh_generator::generate_chunk_meshes,
                mesh_generator::restore_visibility,
            ).chain())
            .insert_resource(MeshGenerator::default())
            .add_event::<RemoveTerrainChildren>();
    }
}

impl SpaceCraftPlugin {
    fn setup(mut commands: Commands) {
        let radius = 4;
        let scale = 2.0f32.powf(radius as f32 - 1.0) * CHUNK_SIZE as f32;
        commands.spawn((
            Camera3d { ..default() },
            Transform::from_translation(Face::Front.direction_vector() * scale + 20.0).looking_at(Vec3::default(), Vec3::Y),
            Projection::Perspective(PerspectiveProjection { ..default() }.into()),
            MainCamera
        ));

        // Spawn the planet entity first so every chunk can reference it;
        // the seed is the planet's terrain signature (distinct per planet
        // once there are several).
        let planet_entity = commands.spawn_empty().id();
        let planet = Planet::new(planet_entity, radius, scale, 0, &mut commands);
        commands.entity(planet_entity).insert(planet);
        commands.spawn((
            DirectionalLight { ..default() },
            Transform::from_translation(Vec3::new(15.0, 15.0, 15.0)),
        ));
    }
}
