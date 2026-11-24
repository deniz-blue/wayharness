use smithay::{
    backend::input::{Axis, AxisSource, ButtonState, KeyState},
    desktop::WindowSurfaceType,
    input::{
        Seat,
        keyboard::{FilterResult, KeyboardHandle, Keycode, Keysym, XkbConfig, keysyms, xkb},
        pointer::{AxisFrame, ButtonEvent, MotionEvent, PointerHandle},
    },
    utils::{Point, SERIAL_COUNTER},
};

use crate::{protocol::Button, state::State};

pub struct Input {
    pointer: PointerHandle<State>,
    keyboard: KeyboardHandle<State>,
    keymap: xkb::Keymap,
}

impl Input {
    pub fn new(seat: &mut Seat<State>) -> Result<Self, Box<dyn std::error::Error>> {
        let keymap = xkb::Keymap::new_from_names(&xkb::Context::new(0), "", "", "", "", None, 0)
            .ok_or("could not compile the default keymap")?;
        Ok(Self {
            pointer: seat.add_pointer(),
            keyboard: seat.add_keyboard(XkbConfig::default(), 200, 25)?,
            keymap,
        })
    }
}

pub fn move_pointer(state: &mut State, id: u32, x: f64, y: f64) -> Result<(), String> {
    let window = &state.find(id).ok_or(format!("no window {id}"))?.window;
    let location = Point::from((x, y)) + window.geometry().loc.to_f64();
    let (surface, origin) = window
        .surface_under(location, WindowSurfaceType::ALL)
        .ok_or(format!("window {id} has no surface at {x},{y}"))?;
    let pointer = state.input.pointer.clone();
    let event = MotionEvent { location, serial: SERIAL_COUNTER.next_serial(), time: state.now_ms() };
    pointer.motion(state, Some((surface, origin.to_f64())), &event);
    pointer.frame(state);
    Ok(())
}

pub fn click(state: &mut State, id: u32, x: f64, y: f64, button: Button) -> Result<(), String> {
    move_pointer(state, id, x, y)?;
    for button_state in [ButtonState::Pressed, ButtonState::Released] {
        let pointer = state.input.pointer.clone();
        let event = ButtonEvent {
            serial: SERIAL_COUNTER.next_serial(),
            time: state.now_ms(),
            button: button.evdev_code(),
            state: button_state,
        };
        pointer.button(state, &event);
        pointer.frame(state);
    }
    Ok(())
}

pub fn scroll(state: &mut State, id: u32, x: f64, y: f64, dx: f64, dy: f64) -> Result<(), String> {
    move_pointer(state, id, x, y)?;
    let pointer = state.input.pointer.clone();
    let frame = AxisFrame::new(state.now_ms())
        .source(AxisSource::Wheel)
        .value(Axis::Horizontal, dx)
        .value(Axis::Vertical, dy);
    pointer.axis(state, frame);
    pointer.frame(state);
    Ok(())
}

pub fn press_keys(state: &mut State, id: u32, keys: &str) -> Result<(), String> {
    let keycodes = keys
        .split('+')
        .map(|name| match xkb::keysym_from_name(name, xkb::KEYSYM_NO_FLAGS) {
            symbol if symbol.raw() == keysyms::KEY_NoSymbol => Err(format!("unknown key {name}")),
            symbol => keycodes_for(&state.input.keymap, symbol),
        })
        .collect::<Result<Vec<_>, _>>()?;
    focus_keyboard(state, id)?;
    tap(state, &keycodes.concat());
    Ok(())
}

pub fn type_text(state: &mut State, id: u32, text: &str) -> Result<(), String> {
    let chords = text
        .chars()
        .map(|c| match c {
            '\n' => keycodes_for(&state.input.keymap, Keysym::new(keysyms::KEY_Return)),
            c => keycodes_for(&state.input.keymap, xkb::utf32_to_keysym(c as u32)),
        })
        .collect::<Result<Vec<_>, _>>()?;
    focus_keyboard(state, id)?;
    chords.iter().for_each(|chord| tap(state, chord));
    Ok(())
}

fn focus_keyboard(state: &mut State, id: u32) -> Result<(), String> {
    let surface = state.find(id).ok_or(format!("no window {id}"))?.window.toplevel().unwrap().wl_surface().clone();
    let keyboard = state.input.keyboard.clone();
    keyboard.set_focus(state, Some(surface), SERIAL_COUNTER.next_serial());
    Ok(())
}

fn keycodes_for(keymap: &xkb::Keymap, symbol: Keysym) -> Result<Vec<Keycode>, String> {
    let (keycode, level) = find_key(keymap, symbol).ok_or(format!("no key produces {}", xkb::keysym_get_name(symbol)))?;
    match level {
        0 => Ok(vec![keycode]),
        _ => {
            let (shift, _) = find_key(keymap, Keysym::new(keysyms::KEY_Shift_L)).ok_or("keymap has no Shift_L")?;
            Ok(vec![shift, keycode])
        }
    }
}

fn find_key(keymap: &xkb::Keymap, symbol: Keysym) -> Option<(Keycode, u32)> {
    (keymap.min_keycode().raw()..=keymap.max_keycode().raw()).map(Keycode::new).find_map(|keycode| {
        let levels = keymap.num_levels_for_key(keycode, 0).min(2);
        (0..levels)
            .find(|level| keymap.key_get_syms_by_level(keycode, 0, *level).contains(&symbol))
            .map(|level| (keycode, level))
    })
}

fn tap(state: &mut State, keycodes: &[Keycode]) {
    for keycode in keycodes {
        send_key(state, *keycode, KeyState::Pressed);
    }
    for keycode in keycodes.iter().rev() {
        send_key(state, *keycode, KeyState::Released);
    }
}

fn send_key(state: &mut State, keycode: Keycode, key_state: KeyState) {
    let keyboard = state.input.keyboard.clone();
    let (serial, time) = (SERIAL_COUNTER.next_serial(), state.now_ms());
    keyboard.input::<(), _>(state, keycode, key_state, serial, time, |_, _, _| FilterResult::Forward);
}
