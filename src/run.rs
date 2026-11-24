use std::{process::{Command, ExitCode}, sync::mpsc, thread};

use smithay::reexports::calloop::LoopSignal;

use crate::serve::{Endpoints, Server};

pub fn run(display: Option<String>, size: (i32, i32), command: &[String]) -> Result<ExitCode, String> {
    let (sender, receiver) = mpsc::channel::<Result<(Endpoints, LoopSignal), String>>();
    let server = thread::spawn(move || match Server::start(display, size) {
        Ok(server) => {
            sender.send(Ok((server.endpoints.clone(), server.signal()))).ok();
            server.run().map_err(|e| eprintln!("{e}")).ok();
        }
        Err(error) => {
            sender.send(Err(error.to_string())).ok();
        }
    });
    let (endpoints, signal) = receiver.recv().map_err(|e| e.to_string())??;
    let mut process = Command::new(&command[0]);
    process.args(&command[1..]).env("WAYLAND_DISPLAY", &endpoints.display).env("WAYHARNESS", &endpoints.display);
    if let Some(bus) = &endpoints.session_bus {
        process.env("DBUS_SESSION_BUS_ADDRESS", bus);
    }
    let status = process.status();
    signal.stop();
    signal.wakeup();
    server.join().ok();
    let status = status.map_err(|e| format!("{}: {e}", command[0]))?;
    Ok(ExitCode::from(status.code().map_or(1, |code| code as u8)))
}
