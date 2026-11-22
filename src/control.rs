use std::{
    fs::File,
    io::{BufRead, BufWriter, Write},
    path::PathBuf,
};

use smithay::utils::Size;

use crate::{render, state::{State, toplevel_strings}};

pub fn name() -> String {
    std::env::var("WAYHARNESS").unwrap_or_else(|_| "wayharness".into())
}

pub fn socket_path() -> PathBuf {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(runtime_dir).join(format!("{}.ctl", name()))
}

pub fn read_request(stream: &std::os::unix::net::UnixStream) -> std::io::Result<String> {
    let mut line = String::new();
    std::io::BufReader::new(stream).read_line(&mut line)?;
    Ok(line.trim_end().to_string())
}

pub fn run(state: &mut State, request: &str) -> Result<String, String> {
    let mut words = request.splitn(3, ' ');
    let command = words.next().unwrap_or_default();
    let numbers = |rest: &str| -> Result<Vec<u32>, String> {
        rest.split(' ').map(|n| n.parse::<u32>().map_err(|e| format!("{n}: {e}"))).collect()
    };
    match command {
        "list" => Ok(list(state)),
        "resize-output" => match numbers(&request["resize-output ".len().min(request.len())..])?[..] {
            [w, h] => {
                state.set_output_size(w as i32, h as i32);
                Ok(String::new())
            }
            _ => Err("usage: resize-output W H".into()),
        },
        "resize-window" => match numbers(&request["resize-window ".len().min(request.len())..])?[..] {
            [id, w, h] => resize_window(state, id, w as i32, h as i32),
            _ => Err("usage: resize-window ID W H".into()),
        },
        "screenshot" => {
            let target = words.next().ok_or("usage: screenshot output|ID PATH")?;
            let path = words.next().ok_or("usage: screenshot output|ID PATH")?;
            let window = match target {
                "output" => None,
                id => Some(id.parse::<u32>().map_err(|e| format!("{id}: {e}"))?),
            };
            screenshot(state, window, path)
        }
        _ => Err(format!("unknown command: {command}")),
    }
}

fn list(state: &State) -> String {
    state
        .toplevels
        .iter()
        .map(|toplevel| {
            let (app_id, title) = toplevel_strings(toplevel.window.toplevel().unwrap());
            let size = toplevel.window.geometry().size;
            format!("{}\t{}\t{}\t{}x{}\n", toplevel.id, app_id, title, size.w, size.h)
        })
        .collect()
}

fn resize_window(state: &State, id: u32, width: i32, height: i32) -> Result<String, String> {
    let toplevel = state.find(id).ok_or(format!("no window {id}"))?;
    let surface = toplevel.window.toplevel().unwrap();
    surface.with_pending_state(|pending| pending.size = Some(Size::from((width, height))));
    surface.send_pending_configure();
    Ok(String::new())
}

fn screenshot(state: &mut State, window: Option<u32>, path: &str) -> Result<String, String> {
    let image = render::render(state, window)?;
    let file = File::create(path).map_err(|e| format!("{path}: {e}"))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&image.rgba).map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())?;
    Ok(String::new())
}

pub fn reply(stream: &mut std::os::unix::net::UnixStream, result: Result<String, String>) {
    let text = match result {
        Ok(text) => text,
        Err(message) => format!("error: {message}\n"),
    };
    stream.write_all(text.as_bytes()).ok();
}
