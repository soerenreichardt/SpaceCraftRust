use bevy::app::{App, Plugin, Startup, Update};
use bevy::prelude::{Camera3dBundle, Commands, Component};
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
            .add_systems(Update, terrain::terrain_quad_tree::update);
    }
}

impl SpaceCraftPlugin {
    fn setup(mut commands: Commands) {
        commands.spawn((
            Camera3dBundle::default(),
            MainCamera
        ));
        commands.spawn(terrain::planet::Planet::new(TerrainQuadTreeConfig { lod_threshold: 1000.0 }));
    }
}