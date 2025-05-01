use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin};
use ::bevy::prelude::*;

use SpaceCraft::SpaceCraftPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(SpaceCraftPlugin)
        .add_plugins((FrameTimeDiagnosticsPlugin::default(), LogDiagnosticsPlugin::default()))
        .run();
}
