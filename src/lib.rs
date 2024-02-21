use bevy::app::{App, Plugin, PreUpdate, Startup, Update};
use bevy::hierarchy::BuildChildren;
use bevy::math::Vec3;
use bevy::prelude::{Camera3dBundle, Commands, IntoSystemConfigs, PerspectiveProjection, ResMut, Transform};
use bevy::utils::default;

use crate::camera::MainCamera;
use crate::terrain::mesh_generator::MeshGenerator;
use crate::terrain::planet::{Face, Planet};

pub(crate) mod terrain;
pub(crate) mod camera;

pub struct SpaceCraftPlugin;

impl Plugin for SpaceCraftPlugin {
    fn build(&self, app: &mut App) {
        app
            .insert_resource(MeshGenerator::new())
            .add_systems(Startup, SpaceCraftPlugin::setup)
            .add_systems(Update, camera::update_camera_position)
            .add_systems(Update, terrain::terrain_quad_tree::update)
            .add_systems(Update, terrain::mesh_generator::update)
            .add_systems(PreUpdate, terrain::terrain_quad_tree::make_new_mesh_visible);
    }
}

impl SpaceCraftPlugin {
    fn setup(mut commands: Commands, mut mesh_generator: ResMut<MeshGenerator>) {
        let radius = 5;
        commands.spawn((
            Camera3dBundle {
                transform: Transform::from_translation(Face::Back.direction_vector() * (8.0 * 16.0 + 20.0)).looking_at(Vec3::default(), Vec3::Y),
                projection: PerspectiveProjection { ..default() }.into(),
                ..default()
            },
            MainCamera
        ));
        Planet::spawn(radius, &mut commands, &mut mesh_generator);
    }
}