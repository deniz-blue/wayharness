mod common;

use common::{Compositor, shm_client::{self, Events, wait_for_event}};
use serde_json::json;

const COLOUR: [u8; 3] = [0x20, 0x80, 0xe0];

fn with_window(compositor: Compositor) -> (Compositor, Events) {
    let events = shm_client::spawn(&compositor.name, COLOUR);
    compositor.wait_for_window_size(64, 48);
    wait_for_event(&events, "frame");
    (compositor, events)
}

fn tail(events: &Events, count: usize) -> Vec<String> {
    let events = events.lock().unwrap();
    events[events.len() - count..].to_vec()
}

#[test]
fn lists_window_as_json() {
    let (compositor, _) = with_window(Compositor::start());
    assert_eq!(
        compositor.windows(),
        [json!({"id": 1, "pid": std::process::id(), "app_id": "wh-test", "title": "hello", "width": 64, "height": 48})]
    );
}

#[test]
fn screenshots_window() {
    let (compositor, _) = with_window(Compositor::start());
    let window = compositor.screenshot(1);
    assert_eq!((window.width, window.height), (64, 48));
    assert_eq!(window.pixel(32, 24), COLOUR);
}

#[test]
fn resizes_window() {
    let (compositor, _) = with_window(Compositor::start());
    compositor.ctl(&["resize-window", "1", "100", "80"]);
    let window = compositor.screenshot(1);
    assert_eq!((window.width, window.height), (100, 80));
    assert_eq!(window.pixel(99, 79), COLOUR);
}

#[test]
fn output_defaults_to_1280x720() {
    let (_compositor, events) = with_window(Compositor::start());
    assert!(events.lock().unwrap().contains(&"output 1280x720".to_string()));
}

#[test]
fn output_size_is_set_at_launch() {
    let (_compositor, events) = with_window(Compositor::start_with_size("800x600"));
    assert!(events.lock().unwrap().contains(&"output 800x600".to_string()));
}

#[test]
fn display_flag_selects_the_server() {
    let (compositor, _) = with_window(Compositor::start());
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_wayharness"))
        .args(["--display", &compositor.name, "list"])
        .env_remove("WAYHARNESS")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout).unwrap().contains("wh-test"));
}

#[test]
fn clicks_at_window_coordinates() {
    let (compositor, events) = with_window(Compositor::start());
    compositor.ctl(&["click", "1", "10", "20"]);
    wait_for_event(&events, "button 272 released");
    assert!(events.lock().unwrap().contains(&"enter 10 20".to_string()));
    assert_eq!(tail(&events, 2), ["button 272 pressed", "button 272 released"]);
    compositor.ctl(&["click", "1", "10", "20", "--button", "right"]);
    wait_for_event(&events, "button 273 released");
    assert_eq!(tail(&events, 2), ["button 273 pressed", "button 273 released"]);
}

#[test]
fn moves_pointer() {
    let (compositor, events) = with_window(Compositor::start());
    compositor.ctl(&["move-pointer", "1", "10", "20"]);
    compositor.ctl(&["move-pointer", "1", "30", "40"]);
    wait_for_event(&events, "motion 30 40");
}

#[test]
fn rejects_point_outside_window() {
    let (compositor, _) = with_window(Compositor::start());
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_wayharness"))
        .args(["move-pointer", "1", "500", "500"])
        .env("WAYHARNESS", &compositor.name)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("no surface at 500,500"));
}

#[test]
fn scrolls() {
    let (compositor, events) = with_window(Compositor::start());
    compositor.ctl(&["scroll", "1", "10", "20", "0", "15"]);
    wait_for_event(&events, "axis vertical 15");
    compositor.ctl(&["scroll", "1", "10", "20", "-3", "0"]);
    wait_for_event(&events, "axis horizontal -3");
}

#[test]
fn presses_keys() {
    let (compositor, events) = with_window(Compositor::start());
    compositor.ctl(&["key", "1", "Return"]);
    wait_for_event(&events, "key 28 released");
    assert!(events.lock().unwrap().contains(&"keyboard enter".to_string()));
    compositor.ctl(&["key", "1", "Control_L+a"]);
    wait_for_event(&events, "key 29 released");
    assert_eq!(
        tail(&events, 4),
        ["key 29 pressed", "key 30 pressed", "key 30 released", "key 29 released"]
    );
}

#[test]
fn types_text() {
    let (compositor, events) = with_window(Compositor::start());
    compositor.ctl(&["type", "1", "Hi"]);
    wait_for_event(&events, "key 23 released");
    assert_eq!(
        tail(&events, 6),
        ["key 42 pressed", "key 35 pressed", "key 35 released", "key 42 released", "key 23 pressed", "key 23 released"]
    );
}
