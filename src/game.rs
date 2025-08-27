use crate::game::ExecuteResult::{Fail, Succ};
use chrono::Utc;
use lazy_static::lazy_static;
use serde::de::{SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, de};
use std::collections::{HashMap, VecDeque};
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
    static ref BUILDIN_VARIABLE: HashMap<String, Box<dyn Fn() -> String + Sync + Send>> = {
        let mut map: HashMap<String, Box<dyn Fn() -> String + Sync + Send>> = HashMap::new();
        map.insert(
            String::from("$YYYY-MM-DD$"),
            Box::new(|| {
                let time = Utc::now();
                time.format("%Y-%m-%d").to_string()
            }),
        );

        map
    };
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

#[derive(Clone)]
struct Expectation(pub Vec<String>);

#[derive(Clone, Deserialize)]
struct Goal {
    kind: GoalKind,
    expectation: Expectation,
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
        let mut root_path = env::current_dir().unwrap();

        // TODO handle unwarps
        // TODO try using a struct to handle directories
        if let Some(environment) = &self.environment {
            let src = root_path.join(&environment.dir);
            if environment.restore {
                let dst = root_path.join(environment.dir.to_string() + TEMP_DIR_POSTFIX);
                copy_dir_all(&src, &dst).unwrap();
                env::set_current_dir(&dst).unwrap();
                restore_path = Some(dst.clone());
                root_path = dst;
            } else {
                env::set_current_dir(&src).unwrap();
                root_path = src;
            }
        }

        for game in &self.game_item {
            println!("{}", game.description);
            println!("{}", t!("helpful_command_hint"));
            let free_mode = self.available_command.len() == 0; // if there's no restrict, do everything you want

            if game.hint_command.is_empty() {
                println!("{}", t!("empty_command_hint"));
            } else {
                for co in &game.hint_command {
                    let guard = COMMAND_HINTS.lock().unwrap();
                    if let Some(hint) = guard.iter().find(|e| e.name.eq(co)) {
                        println!(
                            "{} {}\n{} {}",
                            t!("word_command"),
                            hint.name,
                            t!("word_usage"),
                            hint.hint
                        );
                    }
                }
            }

            loop {
                print!("$ ");
                io::stdout().flush().unwrap();

                let mut input = String::new();
                io::stdin().read_line(&mut input).unwrap(); // hardly fail
                let input = input.trim();
                let input: Vec<String> = input.split(" ").map(|e| e.to_string()).collect();

                if input[0] == EXIT_SYMBOL {
                    break;
                } else if input[0] == HELP_SYMBOL {
                    println!("{}", game.hint);
                } else if input[0] == "" {
                    // means user types Enter
                    continue;
                } else if !self.available_command.contains(&input[0]) && !free_mode {
                    println!(
                        "{}",
                        t!("messages.command_not_supported", command_name = input[0])
                    )
                } else {
                    match self.execute_command(&input, &root_path) {
                        Ok(Succ(output)) => {
                            println!("{output}");
                            if Self::judge_output(game, &input, output) {
                                println!("{}", t!("pass_the_game_item"));
                                break;
                            }
                        }
                        Ok(Fail(output)) => {
                            println!("{}", t!("messages.command_execute_failed", output = output));
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

    fn judge_output(game_item: &GameItem, input: &Vec<String>, output: String) -> bool {
        let goal = &game_item.goal;
        let mut inp = String::new();
        input.iter().for_each(|e| {
            inp += &*e.clone();
            inp += " ";
        });
        let inp = inp.trim().to_string();

        match goal.kind {
            GoalKind::CommandExecuted => goal.expectation.0.contains(&inp),
            GoalKind::StdOut => goal.expectation.0[0] == output,
            GoalKind::DirEntered => env::current_dir()
                .unwrap()
                .ends_with(&game_item.goal.expectation.0[0]),
        }
    }

    fn execute_command(
        &self,
        com: &Vec<String>,
        root: &PathBuf,
    ) -> Result<ExecuteResult<String, String>, Error> {
        if com[0] == "cd" {
            if com.len() == 1 {
                // means the whole command is 'cd', which should get us to the home dir. We won't allow it
                return Ok(Fail(String::from(t!("disallow_home_dir"))));
            } else if com[1].starts_with('/') {
                return Ok(Fail(String::from(t!("disallow_root_dir"))));
            }
            let mut path: PathBuf = env::current_dir()?;
            let mut src: VecDeque<&str> = com[1].split("/").collect();
            while let Some(e) = src.pop_front() {
                match e {
                    "." => (),
                    ".." => {
                        let _ = path.pop();
                    }
                    es => path = path.join(es),
                }
            }

            if root.starts_with(&path) && root != &path {
                println!("{}", t!("get_back_from_bound"));
                env::set_current_dir(root)?;
            } else {
                env::set_current_dir(path)?;
            }

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
            return Ok(Fail(String::from_utf8(output.stderr).unwrap()));
        }

        Ok(Succ(String::from_utf8(output.stdout).unwrap()))
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

impl GamePlayer {
    fn new(mut play_ground: Playground) -> Self {
        let len = *play_ground.game_order.iter().max().unwrap_or(&0) + 1;
        let mut games = vec![None; len];
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

    pub fn play_next(&mut self) -> Result<(), Error> {
        if self.cursor >= self.len {
            return Err(Error::new(ErrorKind::Other, "Already reach the end!"));
        }

        let index = self.play_ground.game_order[self.cursor];
        if let Some(Some(game)) = self.games.get(index) {
            game.play();
            self.cursor += 1;
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
        struct GoalKindVisitor;

        impl<'de> Visitor<'de> for GoalKindVisitor {
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

        deserializer.deserialize_str(GoalKindVisitor)
    }
}

impl<'de> Deserialize<'de> for Expectation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ExpectationVisitor;

        impl<'de> Visitor<'de> for ExpectationVisitor {
            type Value = Expectation;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("a sequence of string needed!")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut expectations = Vec::new();
                while let Some(ele) = seq.next_element::<String>()? {
                    match &*ele {
                        "$YYYY-MM-DD$" => {
                            let time = Utc::now();
                            expectations.push(time.format("%Y-%m-%d").to_string());
                        }
                        _ => {
                            expectations.push(ele);
                        }
                    }
                }

                Ok(Expectation(expectations))
            }
        }

        deserializer.deserialize_str(ExpectationVisitor)
    }
}

// impl<'de> Deserialize<'de> for Goal {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: Deserializer<'de>,
//     {
//         struct GoalVisitor;
//
//         impl<'de> Visitor<'de> for GoalVisitor {
//             type Value = Goal;
//
//             fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
//                 formatter.write_str("Any valid Goal!")
//             }
//
//             fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
//             where
//                 A: MapAccess<'de>,
//             {
//                 let mut kind = None;
//                 let mut expectation = None;
//
//                 while let Some(key) = map.next_key::<String>()? {
//                     match key.as_str() {
//                         "kind" => {
//                             kind = Some(map.next_value::<GoalKind>()?);
//                         }
//                         "expectation" => {
//                             expectation = Some(map.next_value::<Vec<String>>()?)
//                         }
//                         _ => {
//                             let _ = map.next_value::<de::IgnoredAny>();
//                         }
//                     }
//                 }
//
//                 Ok(Goal {
//                     kind: kind.unwrap(),
//                     expectation: expectation.unwrap(),
//                 })
//             }
//
//         }
//
//         deserializer.deserialize_str(GoalVisitor)
//     }
// }

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
        assert_eq!(items[0].goal.expectation.0, vec!["ls", "ls -a"]);
    }

    // #[test]
    // fn game_play_test() {
    //     let game_config_path = "./test/test0.toml";
    //     let player: GamePlayer = GamePlayer::build_from_config(game_config_path).unwrap();
    //     player.play_next().unwrap();
    // }
}
