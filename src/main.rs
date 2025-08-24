use rustyshell::game::GamePlayer;
use std::path::PathBuf;
use std::{env, io};

fn main() -> io::Result<()> {
    println!("\x1b[2J\0"); // clear the console
    let mut working_path: PathBuf = env::current_dir()?;
    working_path.push("./tests_set");
    env::set_current_dir(working_path)?;

    #[cfg(debug_assertions)]
    let game_config_path = "./test/test0.toml";
    #[cfg(not(debug_assertions))]
    let game_config_path = "./game_config.toml";

    let player: GamePlayer = GamePlayer::build_from_config(game_config_path)?;
    player.play_next()?;

    Ok(())
}
