use std::path::PathBuf;

use clap::{Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

#[derive(Subcommand, Serialize, Deserialize)]
pub enum Request {
    List,
    ResizeWindow { id: u32, width: i32, height: i32 },
    Screenshot {
        id: u32,
        #[arg(value_parser = absolute_path)]
        path: PathBuf,
    },
    MovePointer { id: u32, x: f64, y: f64 },
    Click {
        id: u32,
        x: f64,
        y: f64,
        #[arg(long, value_enum, default_value_t = Button::Left)]
        button: Button,
    },
    #[command(allow_negative_numbers = true)]
    Scroll { id: u32, x: f64, y: f64, dx: f64, dy: f64 },
    Key { id: u32, keys: String },
    Type { id: u32, text: String },
}

#[derive(Clone, Copy, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Button {
    Left,
    Right,
    Middle,
}

impl Button {
    pub fn evdev_code(self) -> u32 {
        match self {
            Button::Left => 0x110,
            Button::Right => 0x111,
            Button::Middle => 0x112,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Window {
    pub id: u32,
    pub pid: u32,
    pub app_id: String,
    pub title: String,
    pub width: i32,
    pub height: i32,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Reply {
    Ok(serde_json::Value),
    Error(String),
}

fn absolute_path(path: &str) -> std::io::Result<PathBuf> {
    std::path::absolute(path)
}
