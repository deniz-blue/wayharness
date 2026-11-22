use std::{os::unix::net::UnixListener, sync::Arc, time::Duration};

use smithay::{
    reexports::{
        calloop::{
            EventLoop, Interest, Mode, PostAction,
            generic::Generic,
            timer::{TimeoutAction, Timer},
        },
        wayland_server::Display,
    },
    wayland::socket::ListeningSocketSource,
};

use crate::{
    control,
    state::{ClientState, State},
};

const FRAME_INTERVAL: Duration = Duration::from_millis(16);

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut event_loop: EventLoop<State> = EventLoop::try_new()?;
    let handle = event_loop.handle();
    let display: Display<State> = Display::new()?;
    let mut state = State::new(&display.handle())?;

    let socket = ListeningSocketSource::with_name(&control::name())?;
    handle.insert_source(socket, |stream, _, state| {
        state.display_handle.insert_client(stream, Arc::new(ClientState::default())).ok();
    })?;

    handle.insert_source(Generic::new(display, Interest::READ, Mode::Level), |_, display, state| {
        // Safety: the display is never dropped while the loop runs
        unsafe { display.get_mut().dispatch_clients(state)? };
        Ok(PostAction::Continue)
    })?;

    std::fs::remove_file(control::socket_path()).ok();
    let control_socket = UnixListener::bind(control::socket_path())?;
    handle.insert_source(Generic::new(control_socket, Interest::READ, Mode::Level), |_, listener, state| {
        if let Ok((mut stream, _)) = listener.accept() {
            stream.set_read_timeout(Some(Duration::from_secs(1))).ok();
            if let Ok(request) = control::read_request(&stream) {
                control::reply(&mut stream, control::run(state, &request));
            }
        }
        Ok(PostAction::Continue)
    })?;

    handle.insert_source(Timer::from_duration(FRAME_INTERVAL), |_, _, state| {
        let time = state.start.elapsed();
        for toplevel in &state.toplevels {
            toplevel.window.send_frame(&state.output, time, Some(Duration::ZERO), |_, _| {
                Some(state.output.clone())
            });
        }
        TimeoutAction::ToDuration(FRAME_INTERVAL)
    })?;

    println!("{}", control::name());
    event_loop.run(None, &mut state, |state| {
        state.display_handle.flush_clients().ok();
    })?;
    Ok(())
}
