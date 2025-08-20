use std::io;
use std::io::stdout;
use crossterm::{execute, terminal, cursor};
use crossterm::terminal::ClearType;

fn main() -> io::Result<()> {
    let mut stdout = stdout();
    terminal::enable_raw_mode()?;
    execute!(
        stdout,
        terminal::EnterAlternateScreen,
        terminal::Clear(ClearType::All),
        cursor::MoveTo(0, 0)
    )?;

    loop {
        
    }

    Ok(())
}
