use rustyshell::game::GamePlayer;
use std::io;
use std::io::stdout;

fn main() -> io::Result<()> {
    println!("\x1b[2J\0"); // clear the console

    #[cfg(debug_assertions)]
    let game_config_path = "./test/test0.toml";
    #[cfg(not(debug_assertions))]
    let game_config_path = "./game_config.toml";

    let player: GamePlayer = GamePlayer::build_from_config(game_config_path)?;
    player.play_next()?;

    Ok(())
}
