use serde::de::{Visitor};
use serde::{de, Deserialize, Deserializer};
use std::fmt::Formatter;

#[derive(Deserialize)]
struct PlayGround {
    game_order: String,
    game_description: String,
    game: Vec<Game>,
}

#[derive(Deserialize)]
struct Game {
    id: usize,
    description: String,
    available_command: Vec<String>,
    environment: Option<Environment>,
    game_item: Vec<GameItem>,
}

#[derive(Deserialize)]
struct GameItem {
    description: String,
    hint: String,
    hint_command: Vec<String>,
}

#[derive(Deserialize)]
struct Environment {
    dir: String,
    restore: bool,
}

#[derive(Deserialize)]
struct Goal {
    kind: GoalKind,
    expectation: Vec<String>,
}

type Expectation = Vec<String>;

enum GoalKind {
    CommandExecuted,
    DirEntered,
    StdOut,
}

impl<'de> Deserialize<'de> for GoalKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>
    {
        struct GoalVisitor;

        impl<'de> Visitor<'de> for GoalVisitor {
            type Value = GoalKind;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("Any valid GoalKind!")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error
            {
                match v {
                    "command_executed" => Ok(GoalKind::CommandExecuted),
                    "dir_entered" => Ok(GoalKind::DirEntered),
                    "stdout" => Ok(GoalKind::StdOut),
                    _ => Err(de::Error::custom("Not a valid goal kind!"))
                }
            }
        }

        deserializer.deserialize_str(GoalVisitor)
    }
}
