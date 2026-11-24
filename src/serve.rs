use std::{os::unix::net::{UnixListener, UnixStream}, path::PathBuf, sync::Arc, time::Duration};

use serde::Serialize;
use signal_hook::consts::{SIGINT, SIGTERM};
use smithay::{
    reexports::{
        calloop::{
            EventLoop, Interest, LoopHandle, LoopSignal, Mode, PostAction,
            generic::Generic,
            timer::{TimeoutAction, Timer},
        },
        wayland_server::Display,
    },
    wayland::socket::ListeningSocketSource,
};

use crate::{
    control, frame,
    session_bus::SessionBus,
    state::{ClientState, State},
};

const FRAME_INTERVAL: Duration = Duration::from_millis(16);

#[derive(Clone, Serialize)]
pub struct Endpoints {
    pub display: String,
    pub control: PathBuf,
    pub session_bus: Option<String>,
}

pub struct Server {
    event_loop: EventLoop<'static, State>,
    state: State,
    pub endpoints: Endpoints,
    _session_bus: Option<SessionBus>,
}

pub fn print_and_serve(display: Option<String>, size: (i32, i32)) -> Result<(), String> {
    let server = Server::start(display, size).map_err(|e| e.to_string())?;
    println!("{}", serde_json::to_string(&server.endpoints).map_err(|e| e.to_string())?);
    server.run().map_err(|e| e.to_string())
}

impl Server {
    pub fn start(display: Option<String>, size: (i32, i32)) -> Result<Self, Box<dyn std::error::Error>> {
        let event_loop: EventLoop<State> = EventLoop::try_new()?;
        let wayland_display: Display<State> = Display::new()?;
        let state = State::new(&wayland_display.handle(), size)?;
        let (socket, name) = listen(display.as_deref())?;
        let session_bus = SessionBus::start(&name).map_err(|e| eprintln!("accessibility unavailable: {e}")).ok();
        let endpoints = Endpoints {
            control: control::socket_path(&name),
            session_bus: session_bus.as_ref().map(|bus| bus.address.clone()),
            display: name,
        };
        let handle = event_loop.handle();
        handle.insert_source(socket, |stream, _, state| {
            state.display_handle.insert_client(stream, Arc::new(ClientState::default())).ok();
        })?;
        insert_clients(&handle, wayland_display)?;
        insert_control(&handle, &endpoints.control)?;
        insert_frame_timer(&handle)?;
        insert_shutdown(&handle, event_loop.get_signal())?;
        Ok(Self { event_loop, state, endpoints, _session_bus: session_bus })
    }

    pub fn signal(&self) -> LoopSignal {
        self.event_loop.get_signal()
    }

    pub fn run(mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.event_loop.run(None, &mut self.state, |state| {
            state.display_handle.flush_clients().ok();
        })?;
        Ok(())
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        std::fs::remove_file(&self.endpoints.control).ok();
    }
}

fn listen(display: Option<&str>) -> Result<(ListeningSocketSource, String), Box<dyn std::error::Error>> {
    if let Some(name) = display {
        return Ok((ListeningSocketSource::with_name(name)?, name.into()));
    }
    (0..100)
        .find_map(|n| {
            let name = format!("wayharness-{n}");
            ListeningSocketSource::with_name(&name).ok().map(|socket| (socket, name))
        })
        .ok_or("no free wayharness-N display".into())
}

fn insert_clients(handle: &LoopHandle<'static, State>, display: Display<State>) -> Result<(), Box<dyn std::error::Error>> {
    handle.insert_source(Generic::new(display, Interest::READ, Mode::Level), |_, display, state| {
        // Safety: the display is never dropped while the loop runs
        unsafe { display.get_mut().dispatch_clients(state)? };
        Ok(PostAction::Continue)
    })?;
    Ok(())
}

fn insert_control(handle: &LoopHandle<'static, State>, path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::remove_file(path).ok();
    let listener = UnixListener::bind(path)?;
    handle.insert_source(Generic::new(listener, Interest::READ, Mode::Level), |_, listener, state| {
        if let Ok((stream, _)) = listener.accept() {
            control::serve_connection(state, stream);
        }
        Ok(PostAction::Continue)
    })?;
    Ok(())
}

fn insert_frame_timer(handle: &LoopHandle<'static, State>) -> Result<(), Box<dyn std::error::Error>> {
    handle.insert_source(Timer::from_duration(FRAME_INTERVAL), |_, _, state| {
        let time = state.now_ms();
        for toplevel in &state.toplevels {
            frame::send_frame_callbacks(toplevel.window.toplevel().unwrap().wl_surface(), time);
        }
        TimeoutAction::ToDuration(FRAME_INTERVAL)
    })?;
    Ok(())
}

fn insert_shutdown(handle: &LoopHandle<'static, State>, signal: LoopSignal) -> Result<(), Box<dyn std::error::Error>> {
    let (read, write) = UnixStream::pair()?;
    write.set_nonblocking(true)?;
    for number in [SIGINT, SIGTERM] {
        signal_hook::low_level::pipe::register(number, write.try_clone()?)?;
    }
    handle.insert_source(Generic::new(read, Interest::READ, Mode::Level), move |_, _, _| {
        signal.stop();
        Ok(PostAction::Remove)
    })?;
    Ok(())
}
