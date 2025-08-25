use rust_i18n::{i18n, t};
use rustyshell::game::GamePlayer;
use std::path::PathBuf;
use std::{env, io};

rust_i18n::i18n!("i18n");

fn main() -> io::Result<()> {
    println!("\x1b[2J\0"); // clear the console
    rust_i18n::set_locale("zh-CN");

    let mut working_path: PathBuf = env::current_dir()?;
    working_path.push("./tests_set");
    env::set_current_dir(working_path)?;

    #[cfg(debug_assertions)]
    let game_config_path = "./test/test0.toml";
    #[cfg(not(debug_assertions))]
    let game_config_path = "./game_config.toml";

    let mut player: GamePlayer = GamePlayer::build_from_config(game_config_path)?;
    while player.play_next().is_ok() {}
    println!("{}", t!("pass_all_game"));

    Ok(())
}
