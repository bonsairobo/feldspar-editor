use feldspar_editor::{Config, EditorPlugin};

use bevy::app::prelude::*;

fn main() -> Result<(), ron::Error> {
    let config = Config::read_file("config.ron")?;

    App::build().add_plugin(EditorPlugin::new(config)).run();

    Ok(())
}
