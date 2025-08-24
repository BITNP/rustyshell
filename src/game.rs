use crate::game::ExecuteResult::{Fail, Succ};
use lazy_static::lazy_static;
use serde::de::Visitor;
use serde::{Deserialize, Deserializer, de};
use std::fmt::Formatter;
use std::fs::{DirEntry, File, FileType};
use std::io::{Error, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Mutex;
use std::{env, fs, io};

const EXIT_SYMBOL: &str = "exit";
const HELP_SYMBOL: &str = "help";
const TEMP_DIR_POSTFIX: &str = "__TEMP";

lazy_static! {
    static ref COMMAND_HINTS: Mutex<Vec<CommandHint>> = Mutex::new(Vec::new());
}

#[derive(Deserialize)]
struct Playground {
    game_order: Vec<usize>,
    #[warn(dead_code)]
    game_description: String,
    game: Vec<Game>,
    command_hints: Vec<CommandHint>,
}

#[derive(Deserialize, Clone)]
struct CommandHint {
    name: String,
    hint: String,
}

#[derive(Deserialize, Clone)]
struct Game {
    id: usize,
    description: String,
    available_command: Vec<String>,
    environment: Option<Environment>,
    game_item: Vec<GameItem>,
}

#[derive(Deserialize, Clone)]
struct GameItem {
    description: String,
    hint: String,
    hint_command: Vec<String>,
    goal: Goal,
}

#[derive(Deserialize, Clone)]
struct Environment {
    dir: String,
    restore: bool,
}

#[derive(Deserialize, Clone)]
struct Goal {
    kind: GoalKind,
    expectation: Vec<String>,
}

#[derive(Debug, PartialEq, Clone)]
enum GoalKind {
    CommandExecuted,
    DirEntered,
    StdOut,
}

pub struct GamePlayer {
    play_ground: Playground,
    games: Vec<Option<Game>>,
    len: usize,
    cursor: usize,
}

enum ExecuteResult<S, F> {
    Succ(S),
    Fail(F),
}

impl Game {
    pub fn play(&self) {
        println!("{}", self.description);
        let mut restore_path: Option<PathBuf> = None;

        // TODO handle unwarps
        // TODO try using a struct to handle directories
        if let Some(environment) = &self.environment {
            let curr_path = env::current_dir().unwrap();
            let src = curr_path.join(&environment.dir);
            env::set_current_dir(&src).unwrap();
            if environment.restore {
                println!("111");
                let dst = curr_path.join(environment.dir.to_string() + TEMP_DIR_POSTFIX);
                copy_dir_all(&src, &dst).unwrap();
                env::set_current_dir(&dst).unwrap();
                restore_path = Some(dst);
            }
        }

        for game in &self.game_item {
            println!("{}", game.description);
            println!("Some helpful command and their hint as followed:");

            if game.hint_command.is_empty() {
                println!("There's no hint!");
            } else {
                for co in &game.hint_command {
                    let guard = COMMAND_HINTS.lock().unwrap();
                    let hint = guard.iter().find(|e| e.name.eq(co)).unwrap();

                    println!("COMMAND: {}\nUSAGE: {}", hint.name, hint.hint);
                }
            }

            loop {
                io::stdout().flush().unwrap();
                let mut input = String::new();
                io::stdin().read_line(&mut input).unwrap(); // hardly fail
                let input = input.trim();
                let input: Vec<String> = input.split(" ").map(|e| e.to_string()).collect();

                if input[0] == EXIT_SYMBOL {
                    break;
                } else if input[0] == HELP_SYMBOL {
                    println!("{}", game.hint);
                } else if !self.available_command.contains(&input[0]) {
                    println!(
                        "The command {} is not supported at this part of game!",
                        input[0]
                    );
                } else if input[0] == "\n" {
                    continue;
                } else {
                    match self.execute_command(&input) {
                        Ok(ExecuteResult::Succ(output)) => {
                            println!("{output}");
                            if Self::judge_output(game, &input, output) {
                                println!(
                                    "Great! You have figured out this problem! Let's go to the next one!"
                                );
                                break;
                            }
                        }
                        Ok(ExecuteResult::Fail(output)) => {
                            println!("There's something wrong: {output}");
                        }
                        Err(err) => {
                            println!("{err}")
                        }
                    }
                }
            }
        }

        if let Some(path) = restore_path {
            env::set_current_dir(path.join("..")).unwrap();
            fs::remove_dir_all(&path).unwrap();
        }
    }

    fn handle_cd(input: &Vec<String>) -> io::Result<()> {
        Ok(())
    }

    fn judge_output(game_item: &GameItem, input: &Vec<String>, output: String) -> bool {
        let goal = &game_item.goal;
        let mut inp = String::new();
        input.iter().for_each(|e| {
            inp += &*e.clone();
            inp += " ";
        });
        let inp = inp.trim().to_string();

        match goal.kind {
            GoalKind::CommandExecuted => goal.expectation.contains(&inp),
            GoalKind::StdOut => goal.expectation[0] == output,
            GoalKind::DirEntered => env::current_dir()
                .unwrap()
                .ends_with(&game_item.goal.expectation[0]),
        }
    }

    fn execute_command(&self, com: &Vec<String>) -> Result<ExecuteResult<String, String>, Error> {
        if com[0] == "cd" {
            if com.len() == 1 {
                // means the whole command is 'cd', which should get us to the home dir. We won't allow it
                return Ok(Fail(String::from(
                    "Enter the home dir is not allowed here!",
                )));
            } else if com[1].starts_with('/') {
                return Ok(Fail(String::from(
                    "Enter the root dir is not allowed here!",
                )));
            }
            let path: PathBuf = env::current_dir()?;
            let path = path.join(&com[1]);
            env::set_current_dir(path)?;

            return Ok(Succ(String::new()));
        }

        let mut command = Command::new(&com[0]);
        command.args(&com[1..com.len()]);

        let mut output: Output = command.output()?;
        // the output of 'echo hi' is actually 'hi\n'
        let _ = output
            .stdout
            .pop_if(|e| com[0] == "echo" && *e == '\n' as u8);

        if !output.status.success() {
            return Ok(ExecuteResult::Fail(
                String::from_utf8(output.stderr).unwrap(),
            ));
        }

        Ok(ExecuteResult::Succ(
            String::from_utf8(output.stdout).unwrap(),
        ))
    }
}

fn copy_dir_all<P: AsRef<Path>, PP: AsRef<Path>>(src: P, dst: PP) -> io::Result<()> {
    if dst.as_ref().exists() {
        fs::remove_dir_all(&dst)?;
    }

    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(&src)? {
        let entry: DirEntry = entry?;
        let ty: FileType = entry.file_type()?;
        let target_path = dst.as_ref().join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &target_path)?;
        } else {
            fs::copy(entry.path(), target_path)?;
        }
    }

    Ok(())
}

// impl<'de> Deserialize<'de> for Game {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: Deserializer<'de>,
//     {
//         struct GameVisitor;
//
//         impl<'de> Visitor<'de> for GameVisitor {
//             type Value = Game;
//
//             fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
//                 formatter.write_str("Unable to visit the config file!")
//             }
//         }
//
//         deserializer.deserialize_str(GameVisitor)
//     }
// }

impl GamePlayer {
    fn new(mut play_ground: Playground) -> Self {
        let len = *play_ground.game_order.iter().max().unwrap_or(&0) + 1;
        let mut games = vec![None; len + 1];
        for e in play_ground.game.drain(..) {
            let index = e.id;
            games[index] = Some(e);
        }

        Self {
            play_ground,
            games,
            len,
            cursor: 0,
        }
    }

    pub fn build_from_config<P: AsRef<Path>>(config_path: P) -> Result<Self, Error> {
        let playground = Playground::build(config_path)?;
        Ok(Self::new(playground))
    }

    pub fn play_next(&self) -> Result<(), Error> {
        if self.cursor >= self.len {
            return Err(Error::new(ErrorKind::Other, "Already reach the end!"));
        }

        let index = self.play_ground.game_order[self.cursor];
        if let Some(Some(game)) = self.games.get(index) {
            game.play();
        } else {
            return Err(Error::new(
                ErrorKind::Other,
                format!("Game with id {} doesn't exist!", index),
            ));
        }

        Ok(())
    }
}

impl Playground {
    pub fn build<P: AsRef<Path>>(config_path: P) -> Result<Self, Error> {
        let mut file = File::open(config_path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        let playground: Playground = toml::from_str(&*content).map_err(|e| {
            Error::new(
                ErrorKind::Other,
                format!("Failed to convert toml into Playground with error {e}"),
            )
        })?;
        let mut guard = COMMAND_HINTS.lock().unwrap();
        *guard = playground.command_hints.clone();
        Ok(playground)
    }
}

impl<'de> Deserialize<'de> for GoalKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct GoalVisitor;

        impl<'de> Visitor<'de> for GoalVisitor {
            type Value = GoalKind;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("Any valid GoalKind!")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match v {
                    "command_executed" => Ok(GoalKind::CommandExecuted),
                    "dir_entered" => Ok(GoalKind::DirEntered),
                    "stdout" => Ok(GoalKind::StdOut),
                    _ => Err(de::Error::custom("Not a valid goal kind!")),
                }
            }
        }

        deserializer.deserialize_str(GoalVisitor)
    }
}

#[cfg(test)]
mod tests {
    use crate::game::{GoalKind, Playground};
    use std::fs::File;
    use std::io::Read;

    #[test]
    fn toml_parse_test() {
        let mut test_file = File::open("../tests_set/test/test0.toml").unwrap();
        let mut content = String::new();
        test_file.read_to_string(&mut content).unwrap();
        let playground: Playground = toml::from_str(&*content).unwrap();
        assert_eq!(playground.game_order, vec![1]);
        assert_eq!(playground.game_description, "Having fun with these games!");
        assert_eq!(playground.command_hints[0].name, "ls");
        let game = &playground.game[0];
        assert_eq!(game.id, 1);
        assert_eq!(game.environment.as_ref().unwrap().dir, "set1");
        let items = &game.game_item;
        assert_eq!(items[0].hint_command, vec!["ls"]);
        assert_eq!(items[0].goal.kind, GoalKind::CommandExecuted);
        assert_eq!(items[0].goal.expectation, vec!["ls", "ls -a"]);
    }

    // #[test]
    // fn game_play_test() {
    //     let game_config_path = "./test/test0.toml";
    //     let player: GamePlayer = GamePlayer::build_from_config(game_config_path).unwrap();
    //     player.play_next().unwrap();
    // }
}
