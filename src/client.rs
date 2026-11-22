use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};

use crate::control;

const USAGE: &str = "usage: wayharness serve
       wayharness list
       wayharness resize-output W H
       wayharness resize-window ID W H
       wayharness screenshot [--window ID] PATH";

fn request(line: &str) -> Result<String, String> {
    let path = control::socket_path();
    let mut stream = UnixStream::connect(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    writeln!(stream, "{line}").map_err(|e| e.to_string())?;
    let mut reply = String::new();
    stream.read_to_string(&mut reply).map_err(|e| e.to_string())?;
    match reply.strip_prefix("error: ") {
        Some(message) => Err(message.trim_end().to_string()),
        None => Ok(reply),
    }
}

fn wait_for_size(id: &str, width: &str, height: &str) -> Result<(), String> {
    let wanted = format!("{width}x{height}");
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let listing = request("list")?;
        let size = listing.lines().find_map(|line| {
            let columns: Vec<_> = line.split('\t').collect();
            (columns[0] == id).then(|| columns[3].to_string())
        });
        if size.as_deref() == Some(&wanted) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Err(format!("window {id} did not become {wanted}"))
}

fn dispatch(args: &[&str]) -> Result<String, String> {
    match args {
        ["list"] => request("list"),
        ["resize-output", w, h] => request(&format!("resize-output {w} {h}")),
        ["resize-window", id, w, h] => {
            request(&format!("resize-window {id} {w} {h}"))?;
            wait_for_size(id, w, h).map(|_| String::new())
        }
        ["screenshot", path] => request(&format!("screenshot output {}", absolute(path)?)),
        ["screenshot", "--window", id, path] => request(&format!("screenshot {id} {}", absolute(path)?)),
        _ => Err(USAGE.into()),
    }
}

fn absolute(path: &str) -> Result<String, String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    Ok(cwd.join(path).display().to_string())
}

pub fn run(args: &[String]) -> std::process::ExitCode {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match dispatch(&args) {
        Ok(output) => {
            print!("{output}");
            std::process::ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            std::process::ExitCode::FAILURE
        }
    }
}
