use bevy::prelude::*;

fn main() -> AppExit {
    App::new().add_plugins((MinimalPlugins, game::server)).run()
}
