use std::time::Instant;

use smithay::{
    backend::renderer::pixman::PixmanRenderer,
    delegate_compositor, delegate_output, delegate_seat, delegate_shm, delegate_xdg_decoration,
    delegate_xdg_shell,
    desktop::Window,
    input::{SeatHandler, SeatState},
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::{
        wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode as DecorationMode,
        wayland_server::{
            Client, DisplayHandle,
            backend::{ClientData, ClientId, DisconnectReason},
            protocol::{wl_buffer::WlBuffer, wl_seat::WlSeat, wl_surface::WlSurface},
        },
    },
    utils::Serial,
    wayland::{
        buffer::BufferHandler,
        compositor::{
            CompositorClientState, CompositorHandler, CompositorState, get_parent, with_states,
        },
        output::{OutputHandler, OutputManagerState},
        shell::xdg::{
            PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
            XdgToplevelSurfaceData,
            decoration::{XdgDecorationHandler, XdgDecorationState},
        },
        shm::{ShmHandler, ShmState},
    },
};

pub struct Toplevel {
    pub id: u32,
    pub window: Window,
}

pub struct State {
    pub display_handle: DisplayHandle,
    pub start: Instant,
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub shm_state: ShmState,
    pub seat_state: SeatState<State>,
    pub output: Output,
    pub toplevels: Vec<Toplevel>,
    pub next_id: u32,
    pub renderer: PixmanRenderer,
}

#[derive(Default)]
pub struct ClientState {
    compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _: ClientId) {}
    fn disconnected(&self, _: ClientId, _: DisconnectReason) {}
}

impl State {
    pub fn new(display: &DisplayHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let output = Output::new(
            "wayharness-0".into(),
            PhysicalProperties {
                size: (0, 0).into(),
                subpixel: Subpixel::Unknown,
                make: "wayharness".into(),
                model: "virtual".into(),
            },
        );
        output.create_global::<State>(display);
        OutputManagerState::new_with_xdg_output::<State>(display);
        XdgDecorationState::new::<State>(display);
        let mut seat_state = SeatState::new();
        seat_state.new_wl_seat(display, "wayharness-seat");
        let state = Self {
            display_handle: display.clone(),
            start: Instant::now(),
            compositor_state: CompositorState::new::<State>(display),
            xdg_shell_state: XdgShellState::new::<State>(display),
            shm_state: ShmState::new::<State>(display, vec![]),
            seat_state,
            output,
            toplevels: Vec::new(),
            next_id: 1,
            renderer: PixmanRenderer::new()?,
        };
        state.set_output_size(1280, 720);
        Ok(state)
    }

    pub fn set_output_size(&self, width: i32, height: i32) {
        let mode = Mode { size: (width, height).into(), refresh: 60_000 };
        self.output.change_current_state(Some(mode), None, None, Some((0, 0).into()));
        self.output.set_preferred(mode);
    }

    pub fn find(&self, id: u32) -> Option<&Toplevel> {
        self.toplevels.iter().find(|toplevel| toplevel.id == id)
    }
}

pub fn toplevel_strings(toplevel: &ToplevelSurface) -> (String, String) {
    with_states(toplevel.wl_surface(), |states| {
        let data = states.data_map.get::<XdgToplevelSurfaceData>().unwrap().lock().unwrap();
        (data.app_id.clone().unwrap_or_default(), data.title.clone().unwrap_or_default())
    })
}

impl CompositorHandler for State {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client.get_data::<ClientState>().unwrap().compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        smithay::backend::renderer::utils::on_commit_buffer_handler::<Self>(surface);
        let mut root = surface.clone();
        while let Some(parent) = get_parent(&root) {
            root = parent;
        }
        let Some(toplevel) = self
            .toplevels
            .iter()
            .find(|toplevel| toplevel.window.toplevel().is_some_and(|t| *t.wl_surface() == root))
        else {
            return;
        };
        toplevel.window.on_commit();
        let top = toplevel.window.toplevel().unwrap();
        let configured = with_states(&root, |states| {
            states.data_map.get::<XdgToplevelSurfaceData>().unwrap().lock().unwrap().initial_configure_sent
        });
        if !configured {
            top.send_configure();
        }
    }
}

impl BufferHandler for State {
    fn buffer_destroyed(&mut self, _: &WlBuffer) {}
}

impl ShmHandler for State {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

impl OutputHandler for State {}

impl SeatHandler for State {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<State> {
        &mut self.seat_state
    }
}

impl XdgDecorationHandler for State {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        self.request_mode(toplevel, DecorationMode::ServerSide);
    }

    fn request_mode(&mut self, toplevel: ToplevelSurface, _: DecorationMode) {
        toplevel.with_pending_state(|state| state.decoration_mode = Some(DecorationMode::ServerSide));
        if toplevel.is_initial_configure_sent() {
            toplevel.send_pending_configure();
        }
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        self.request_mode(toplevel, DecorationMode::ServerSide);
    }
}

impl XdgShellHandler for State {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let id = self.next_id;
        self.next_id += 1;
        self.toplevels.push(Toplevel { id, window: Window::new_wayland_window(surface) });
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        self.toplevels.retain(|toplevel| toplevel.window.toplevel() != Some(&surface));
    }

    fn new_popup(&mut self, _: PopupSurface, _: PositionerState) {}

    fn grab(&mut self, _: PopupSurface, _: WlSeat, _: Serial) {}

    fn reposition_request(&mut self, _: PopupSurface, _: PositionerState, _: u32) {}
}

delegate_compositor!(State);
delegate_seat!(State);
delegate_shm!(State);
delegate_output!(State);
delegate_xdg_shell!(State);
delegate_xdg_decoration!(State);
