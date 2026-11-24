use std::{
    io::Write,
    os::{fd::AsFd, unix::net::UnixStream},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use wayland_client::{
    Connection, Dispatch, QueueHandle, WEnum,
    globals::{GlobalListContents, registry_queue_init},
    protocol::{
        wl_buffer::WlBuffer,
        wl_callback::{self, WlCallback},
        wl_compositor::WlCompositor,
        wl_keyboard::{self, WlKeyboard},
        wl_output::{self, WlOutput},
        wl_pointer::{self, WlPointer},
        wl_seat::{self, WlSeat},
        wl_registry::WlRegistry,
        wl_shm::{Format, WlShm},
        wl_shm_pool::WlShmPool,
        wl_surface::WlSurface,
    },
};
use wayland_protocols::xdg::shell::client::{
    xdg_surface::{self, XdgSurface},
    xdg_toplevel::{self, XdgToplevel},
    xdg_wm_base::{self, XdgWmBase},
};

struct Client {
    surface: WlSurface,
    shm: WlShm,
    colour: [u8; 3],
    size: (i32, i32),
    events: Events,
}

pub type Events = Arc<Mutex<Vec<String>>>;

pub fn wait_for_event(events: &Events, wanted: &str) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let seen = events.lock().unwrap().clone();
        if seen.iter().any(|event| event == wanted) {
            return seen;
        }
        assert!(Instant::now() < deadline, "no {wanted:?}, saw {seen:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub fn spawn(socket_name: &str, colour: [u8; 3]) -> Events {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap();
    let stream = UnixStream::connect(format!("{runtime_dir}/{socket_name}")).unwrap();
    let connection = Connection::from_socket(stream).unwrap();
    let (globals, mut queue) = registry_queue_init::<Client>(&connection).unwrap();
    let qh = queue.handle();
    let compositor: WlCompositor = globals.bind(&qh, 1..=4, ()).unwrap();
    let wm_base: XdgWmBase = globals.bind(&qh, 1..=1, ()).unwrap();
    let surface = compositor.create_surface(&qh, ());
    let toplevel = wm_base.get_xdg_surface(&surface, &qh, ()).get_toplevel(&qh, ());
    toplevel.set_app_id("wh-test".into());
    toplevel.set_title("hello".into());
    surface.commit();
    let _: WlOutput = globals.bind(&qh, 1..=4, ()).unwrap();
    let _: WlSeat = globals.bind(&qh, 1..=7, ()).unwrap();
    let events = Events::default();
    let mut client =
        Client { surface, shm: globals.bind(&qh, 1..=1, ()).unwrap(), colour, size: (64, 48), events: events.clone() };
    std::thread::spawn(move || while queue.blocking_dispatch(&mut client).is_ok() {});
    events
}

impl Client {
    fn draw(&self, qh: &QueueHandle<Self>) {
        let (width, height) = self.size;
        let [r, g, b] = self.colour;
        let pixels: Vec<u8> = (0..width * height).flat_map(|_| [b, g, r, 0xff]).collect();
        let mut file = tempfile_in_runtime_dir();
        file.write_all(&pixels).unwrap();
        let pool = self.shm.create_pool(file.as_fd(), pixels.len() as i32, qh, ());
        let buffer = pool.create_buffer(0, width, height, width * 4, Format::Argb8888, qh, ());
        pool.destroy();
        self.surface.frame(qh, ());
        self.surface.attach(Some(&buffer), 0, 0);
        self.surface.damage_buffer(0, 0, width, height);
        self.surface.commit();
    }
}

fn tempfile_in_runtime_dir() -> std::fs::File {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap();
    let path = format!("{runtime_dir}/wh-shm-{}-{:?}", std::process::id(), std::thread::current().id());
    let file = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(true).open(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    file
}

impl Client {
    fn log(&self, event: String) {
        self.events.lock().unwrap().push(event);
    }
}

fn pressed(down: bool) -> &'static str {
    if down { "pressed" } else { "released" }
}

impl Dispatch<WlOutput, ()> for Client {
    fn event(state: &mut Self, _: &WlOutput, event: wl_output::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let wl_output::Event::Mode { width, height, .. } = event {
            state.log(format!("output {width}x{height}"));
        }
    }
}

impl Dispatch<WlCallback, ()> for Client {
    fn event(state: &mut Self, _: &WlCallback, event: wl_callback::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let wl_callback::Event::Done { .. } = event {
            state.log("frame".into());
        }
    }
}

impl Dispatch<WlSeat, ()> for Client {
    fn event(_: &mut Self, seat: &WlSeat, event: wl_seat::Event, _: &(), _: &Connection, qh: &QueueHandle<Self>) {
        if let wl_seat::Event::Capabilities { capabilities: WEnum::Value(capabilities) } = event {
            if capabilities.contains(wl_seat::Capability::Pointer) {
                seat.get_pointer(qh, ());
            }
            if capabilities.contains(wl_seat::Capability::Keyboard) {
                seat.get_keyboard(qh, ());
            }
        }
    }
}

impl Dispatch<WlPointer, ()> for Client {
    fn event(state: &mut Self, _: &WlPointer, event: wl_pointer::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match event {
            wl_pointer::Event::Enter { surface_x, surface_y, .. } => state.log(format!("enter {surface_x} {surface_y}")),
            wl_pointer::Event::Motion { surface_x, surface_y, .. } => state.log(format!("motion {surface_x} {surface_y}")),
            wl_pointer::Event::Button { button, state: button_state, .. } => {
                let down = matches!(button_state, WEnum::Value(wl_pointer::ButtonState::Pressed));
                state.log(format!("button {button} {}", pressed(down)));
            }
            wl_pointer::Event::Axis { axis, value, .. } => {
                let name = if matches!(axis, WEnum::Value(wl_pointer::Axis::VerticalScroll)) { "vertical" } else { "horizontal" };
                state.log(format!("axis {name} {value}"));
            }
            _ => {}
        }
    }
}

impl Dispatch<WlKeyboard, ()> for Client {
    fn event(state: &mut Self, _: &WlKeyboard, event: wl_keyboard::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match event {
            wl_keyboard::Event::Enter { .. } => state.log("keyboard enter".into()),
            wl_keyboard::Event::Key { key, state: key_state, .. } => {
                let down = matches!(key_state, WEnum::Value(wl_keyboard::KeyState::Pressed));
                state.log(format!("key {key} {}", pressed(down)));
            }
            _ => {}
        }
    }
}

impl Dispatch<XdgSurface, ()> for Client {
    fn event(state: &mut Self, surface: &XdgSurface, event: xdg_surface::Event, _: &(), _: &Connection, qh: &QueueHandle<Self>) {
        if let xdg_surface::Event::Configure { serial } = event {
            surface.ack_configure(serial);
            state.draw(qh);
        }
    }
}

impl Dispatch<XdgToplevel, ()> for Client {
    fn event(state: &mut Self, _: &XdgToplevel, event: xdg_toplevel::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let xdg_toplevel::Event::Configure { width, height, .. } = event {
            if width > 0 && height > 0 {
                state.size = (width, height);
            }
        }
    }
}

impl Dispatch<XdgWmBase, ()> for Client {
    fn event(_: &mut Self, base: &XdgWmBase, event: xdg_wm_base::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            base.pong(serial);
        }
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for Client {
    fn event(_: &mut Self, _: &WlRegistry, _: wayland_client::protocol::wl_registry::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
}

macro_rules! ignore_events {
    ($($interface:ty),*) => {$(
        impl Dispatch<$interface, ()> for Client {
            fn event(_: &mut Self, _: &$interface, _: <$interface as wayland_client::Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
        }
    )*};
}
ignore_events!(WlCompositor, WlShm, WlShmPool, WlBuffer, WlSurface);
