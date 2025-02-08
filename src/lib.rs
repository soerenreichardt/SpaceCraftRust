use bevy::app::{App, Plugin, Startup, Update};
use bevy::asset::Assets;
use bevy::hierarchy::BuildChildren;
use bevy::math::Vec3;
use bevy::pbr::{DirectionalLightBundle, StandardMaterial};
use bevy::prelude::{Camera3dBundle, Commands, IntoSystemConfigs, Mesh, PerspectiveProjection, ResMut, Transform};
use bevy::utils::default;

use crate::camera::MainCamera;
use crate::terrain::mesh_generator::MeshGenerator2;
use crate::terrain::mesh_generator::MESH_SIZE;
use crate::terrain::planet::Planet;
use crate::terrain::Face;

pub(crate) mod terrain;
pub(crate) mod camera;

pub struct SpaceCraftPlugin;

impl Plugin for SpaceCraftPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<MeshGenerator2>()
            .add_systems(Startup, SpaceCraftPlugin::setup)
            .add_systems(Update, camera::update_camera_position)
            .add_systems(Update, terrain::planet::update_lod);
    }
}

impl SpaceCraftPlugin {
    fn setup(mut commands: Commands, mut mesh_generator: ResMut<MeshGenerator2>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
        let radius = 4;
        commands.spawn((
            Camera3dBundle {
                transform: Transform::from_translation(Face::Back.direction_vector() * (8.0 * 16.0 + 20.0)).looking_at(Vec3::default(), Vec3::Y),
                projection: PerspectiveProjection { ..default() }.into(),
                ..default()
            },
            MainCamera
        ));
        let scale = 2.0f32.powf(radius as f32 - 1.0) * MESH_SIZE as f32;
        let planet = Planet::new(radius, scale, &mut commands, &mut mesh_generator, &mut meshes, &mut materials);
        commands.spawn(planet);
        commands.spawn(DirectionalLightBundle {
            transform: Transform::from_translation(Vec3::new(15.0, 15.0, 15.0)),
            ..default()
        });
    }
}
