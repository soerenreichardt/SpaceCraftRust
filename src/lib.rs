use bevy::app::{App, Plugin, Startup, Update};
use bevy::math::Vec3;
use bevy::prelude::{Camera3dBundle, Commands, PerspectiveProjection, ResMut, Transform};
use bevy::utils::default;

use crate::camera::MainCamera;
use crate::terrain::mesh_generator::MeshGenerator;
use crate::terrain::planet::Planet;

pub(crate) mod terrain;
pub(crate) mod camera;

pub struct SpaceCraftPlugin;

impl Plugin for SpaceCraftPlugin {
    fn build(&self, app: &mut App) {
        app
            .insert_resource(MeshGenerator::new())
            .add_systems(Startup, SpaceCraftPlugin::setup)
            .add_systems(Update, (terrain::terrain_quad_tree::update, camera::update_camera_position, terrain::mesh_generator::update));
    }
}

impl SpaceCraftPlugin {
    fn setup(mut commands: Commands, mut mesh_generator: ResMut<MeshGenerator>) {
        commands.spawn((
            Camera3dBundle {
                transform: Transform::from_xyz(0.0, 0.0, 8.0).looking_at(Vec3::default(), Vec3::Y),
                projection: PerspectiveProjection { ..default() }.into(),
                ..default()
            },
            MainCamera
        ));
        Planet::spawn(&mut commands, &mut mesh_generator);
    }
}