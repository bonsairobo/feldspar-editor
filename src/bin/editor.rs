use feldspar_editor::{Config, EditorPlugin};

use bevy::app::prelude::*;

fn main() -> Result<(), ron::Error> {
    env_logger::init();

    let config = Config::read_file("config.ron")?;

    App::build().add_plugin(EditorPlugin::new(config)).run();

    Ok(())
}
