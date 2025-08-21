use serde::de::Visitor;
use serde::{Deserialize, Deserializer, de};
use std::fmt::Formatter;
use std::fs::File;
use std::io::{Error, ErrorKind, Read};
use std::path::Path;

#[derive(Deserialize)]
struct Playground {
    game_order: Vec<usize>,
    game_description: String,
    game: Vec<Game>,
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

type Expectation = Vec<String>;

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

impl Game {
    pub(crate) fn play(&self) {
        println!("{}", self.description);
    }
}

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
    use crate::game::{GamePlayer, GoalKind, Playground};
    use std::fs::File;
    use std::io::Read;

    #[test]
    fn toml_parse_test() {
        let mut test_file = File::open("./test/test0.toml").unwrap();
        let mut content = String::new();
        test_file.read_to_string(&mut content).unwrap();
        let playground: Playground = toml::from_str(&*content).unwrap();
        assert_eq!(playground.game_order, vec![1]);
        assert_eq!(playground.game_description, "Having fun with these games!");
        let game = &playground.game[0];
        assert_eq!(game.id, 1);
        assert_eq!(game.environment.as_ref().unwrap().dir, "set1");
        let items = &game.game_item;
        assert_eq!(items[0].hint_command, vec!["ls"]);
        assert_eq!(items[0].goal.kind, GoalKind::CommandExecuted);
        assert_eq!(items[0].goal.expectation, vec!["ls", "ls -a"]);
    }

    #[test]
    fn game_play_test() {
        let game_config_path = "./test/test0.toml";
        let player: GamePlayer = GamePlayer::build_from_config(game_config_path).unwrap();
        player.play_next().unwrap();
    }
}
