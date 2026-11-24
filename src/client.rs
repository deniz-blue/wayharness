use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};

use crate::{
    control,
    protocol::{Reply, Request, Window},
};

pub fn require_display(display: Option<String>) -> Result<String, String> {
    display.ok_or("no display: pass --display or set WAYHARNESS".into())
}

pub fn windows(display: &str) -> Result<Vec<Window>, String> {
    serde_json::from_value(call(display, &Request::List)?).map_err(|e| e.to_string())
}

pub fn send(display: Option<String>, request: Request) -> Result<(), String> {
    let display = require_display(display)?;
    let value = call(&display, &request)?;
    if let Request::ResizeWindow { id, width, height } = request {
        wait_for_size(&display, id, (width, height))?;
    }
    if !value.is_null() {
        println!("{value}");
    }
    Ok(())
}

fn call(display: &str, request: &Request) -> Result<serde_json::Value, String> {
    let path = control::socket_path(display);
    let mut stream = UnixStream::connect(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let line = serde_json::to_string(request).map_err(|e| e.to_string())?;
    writeln!(stream, "{line}").map_err(|e| e.to_string())?;
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply).map_err(|e| e.to_string())?;
    match serde_json::from_str(&reply).map_err(|e| format!("bad reply: {e}"))? {
        Reply::Ok(value) => Ok(value),
        Reply::Error(message) => Err(message),
    }
}

fn wait_for_size(display: &str, id: u32, (width, height): (i32, i32)) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if windows(display)?.iter().any(|w| w.id == id && (w.width, w.height) == (width, height)) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Err(format!("window {id} did not become {width}x{height}"))
}
