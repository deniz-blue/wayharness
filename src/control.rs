use std::{
    fs::File,
    io::{BufRead, BufReader, BufWriter, Write},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    time::Duration,
};

use serde_json::{Value, json};
use smithay::{reexports::wayland_server::Resource, utils::Size};

use crate::{
    input, render,
    protocol::{Reply, Request, Window},
    state::{State, Toplevel, toplevel_strings},
};

pub fn runtime_dir() -> PathBuf {
    std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into()).into()
}

pub fn socket_path(display: &str) -> PathBuf {
    runtime_dir().join(format!("{display}.ctl"))
}

pub fn serve_connection(state: &mut State, mut stream: UnixStream) {
    stream.set_read_timeout(Some(Duration::from_secs(1))).ok();
    let mut line = String::new();
    if BufReader::new(&stream).read_line(&mut line).is_err() {
        return;
    }
    let reply = match serde_json::from_str(&line) {
        Ok(request) => handle(state, request).map_or_else(Reply::Error, Reply::Ok),
        Err(error) => Reply::Error(error.to_string()),
    };
    if let Ok(text) = serde_json::to_string(&reply) {
        writeln!(stream, "{text}").ok();
    }
}

fn handle(state: &mut State, request: Request) -> Result<Value, String> {
    let done = match request {
        Request::List => return Ok(json!(windows(state))),
        Request::ResizeWindow { id, width, height } => resize_window(state, id, width, height),
        Request::Screenshot { id, path } => screenshot(state, id, &path),
        Request::MovePointer { id, x, y } => input::move_pointer(state, id, x, y),
        Request::Click { id, x, y, button } => input::click(state, id, x, y, button),
        Request::Scroll { id, x, y, dx, dy } => input::scroll(state, id, x, y, dx, dy),
        Request::Key { id, keys } => input::press_keys(state, id, &keys),
        Request::Type { id, text } => input::type_text(state, id, &text),
    };
    done.map(|()| Value::Null)
}

fn windows(state: &State) -> Vec<Window> {
    state
        .toplevels
        .iter()
        .map(|toplevel| {
            let (app_id, title) = toplevel_strings(toplevel.window.toplevel().unwrap());
            let size = toplevel.window.geometry().size;
            Window { id: toplevel.id, pid: process_id(state, toplevel), app_id, title, width: size.w, height: size.h }
        })
        .collect()
}

fn process_id(state: &State, toplevel: &Toplevel) -> u32 {
    let surface = toplevel.window.toplevel().unwrap().wl_surface();
    surface
        .client()
        .and_then(|client| client.get_credentials(&state.display_handle).ok())
        .map_or(0, |credentials| credentials.pid as u32)
}

fn resize_window(state: &State, id: u32, width: i32, height: i32) -> Result<(), String> {
    let toplevel = state.find(id).ok_or(format!("no window {id}"))?;
    let surface = toplevel.window.toplevel().unwrap();
    surface.with_pending_state(|pending| pending.size = Some(Size::from((width, height))));
    surface.send_pending_configure();
    Ok(())
}

fn screenshot(state: &mut State, id: u32, path: &Path) -> Result<(), String> {
    let image = render::render(state, id)?;
    let file = File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&image.rgba).map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())
}
