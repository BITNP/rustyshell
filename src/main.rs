use std::io;
use std::io::stdout;
use crossterm::{execute, terminal, cursor};
use crossterm::terminal::ClearType;
use rustyshell::game::GamePlayer;

fn main() -> io::Result<()> {
    let mut stdout = stdout();
    terminal::enable_raw_mode()?;
    execute!(
        stdout,
        terminal::EnterAlternateScreen,
        terminal::Clear(ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    
    #[cfg(debug_assertions)]
    let game_config_path = "./test/test0.toml";
    #[cfg(not(debug_assertions))]
    let game_config_path = "./game_config.toml";
    
    let player: GamePlayer = GamePlayer::build_from_config(game_config_path)?;
    player.play_next()?;

    Ok(())
}
