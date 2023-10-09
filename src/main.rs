use::bevy::prelude::*;

use SpaceCraft::SpaceCraftPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(SpaceCraftPlugin)
        .run();
}
