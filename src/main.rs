use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin};
use ::bevy::prelude::*;
use bevy::pbr::wireframe::WireframePlugin;
use SpaceCraft::SpaceCraftPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(SpaceCraftPlugin)
        .add_plugins((FrameTimeDiagnosticsPlugin::default(), LogDiagnosticsPlugin::default()))
        .add_plugins(WireframePlugin::default())
        .run();
}
