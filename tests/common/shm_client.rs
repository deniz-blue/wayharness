use std::{io::Write, os::unix::net::UnixStream, os::fd::AsFd};

use wayland_client::{
    Connection, Dispatch, QueueHandle,
    globals::{GlobalListContents, registry_queue_init},
    protocol::{
        wl_buffer::WlBuffer,
        wl_compositor::WlCompositor,
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
}

pub fn spawn(socket_name: &str, colour: [u8; 3]) {
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
    let mut client = Client { surface, shm: globals.bind(&qh, 1..=1, ()).unwrap(), colour, size: (64, 48) };
    std::thread::spawn(move || while queue.blocking_dispatch(&mut client).is_ok() {});
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
