use bevy::app::{App, Plugin, Startup, Update};
use bevy::hierarchy::BuildChildren;
use bevy::math::Vec3;
use bevy::pbr::DirectionalLightBundle;
use bevy::prelude::{Camera3dBundle, Commands, IntoSystemConfigs, PerspectiveProjection, Transform};
use bevy::utils::default;

use crate::camera::MainCamera;
use crate::terrain::mesh_generator::MeshGenerator;
use crate::terrain::mesh_generator::MESH_SIZE;
use crate::terrain::planet::Planet;
use crate::terrain::Face;
use crate::terrain::terrain_quad_tree::RemoveTerrainChildren;

pub(crate) mod terrain;
pub(crate) mod camera;

pub struct SpaceCraftPlugin;

impl Plugin for SpaceCraftPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<MeshGenerator>()
            .add_systems(Startup, SpaceCraftPlugin::setup)
            .add_systems(Update, camera::update_camera_position)
            .add_systems(Update, terrain::planet::update_lod)
            .add_systems(Update, terrain::mesh_generator::generate_chunk_meshes)
            .add_systems(Update, terrain::mesh_generator::restore_visibility)
            .add_event::<RemoveTerrainChildren>();
    }
}

impl SpaceCraftPlugin {
    fn setup(mut commands: Commands) {
        let radius = 10;
        let scale = 2.0f32.powf(radius as f32 - 1.0) * MESH_SIZE as f32;
        commands.spawn((
            Camera3dBundle {
                transform: Transform::from_translation(Face::Back.direction_vector() * scale + 20.0).looking_at(Vec3::default(), Vec3::Y),
                projection: PerspectiveProjection { ..default() }.into(),
                ..default()
            },
            MainCamera
        ));

        let planet = Planet::new(radius, scale, &mut commands);
        commands.spawn(planet);
        commands.spawn(DirectionalLightBundle {
            transform: Transform::from_translation(Vec3::new(15.0, 15.0, 15.0)),
            ..default()
        });
    }
}
