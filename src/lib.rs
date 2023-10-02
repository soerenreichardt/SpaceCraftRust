use bevy::app::{App, Plugin, Startup, Update};
use bevy::asset::Assets;
use bevy::math::Vec3;
use bevy::pbr::{PbrBundle, StandardMaterial};
use bevy::prelude::{Camera3dBundle, Color, Commands, Component, Mesh, OrthographicProjection, ResMut, Transform};
use bevy::prelude::shape::Icosphere;
use bevy::utils::default;
use crate::terrain::planet::Planet;

use crate::terrain::terrain_quad_tree::TerrainQuadTreeConfig;

pub(crate) mod datastructures;
pub(crate) mod terrain;

pub struct SpaceCraftPlugin;

#[derive(Component)]
pub struct MainCamera;

impl Plugin for SpaceCraftPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(Startup, SpaceCraftPlugin::setup)
            .add_systems(Update, terrain::planet::update);
    }
}

impl SpaceCraftPlugin {
    fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
        commands.spawn((
            Camera3dBundle {
                transform: Transform::from_xyz(0.0, 0.0, 8.0).looking_at(Vec3::default(), Vec3::Y),
                projection: OrthographicProjection {
                    scale: 0.01,
                    ..default()
                }
                    .into(),
                ..default()
            },
            MainCamera
        ));
        Planet::spawn(TerrainQuadTreeConfig { lod_threshold: 1.0 }, &mut commands, &mut meshes, &mut materials);
    }
}