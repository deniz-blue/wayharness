mod common;

use std::{path::PathBuf, process::Command};

use common::Compositor;

fn runtime_dir() -> PathBuf {
    std::env::var("XDG_RUNTIME_DIR").unwrap().into()
}

#[test]
fn serve_without_a_name_picks_free_displays() {
    let first = Compositor::spawn(&["serve"], None);
    let second = Compositor::spawn(&["serve"], None);
    assert!(first.name.starts_with("wayharness-"));
    assert!(second.name.starts_with("wayharness-"));
    assert_ne!(first.name, second.name);
    assert!(runtime_dir().join(&first.name).exists());
    assert!(runtime_dir().join(format!("{}.ctl", first.name)).exists());
    assert_eq!(first.ctl(&["list"]), "[]\n");
}

#[test]
fn display_flag_names_the_server() {
    let name = format!("wh-test-flag-{}", std::process::id());
    let compositor = Compositor::spawn(&["--display", &name, "serve"], None);
    assert_eq!(compositor.name, name);
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_wayharness"))
        .arg("run")
        .args(args)
        .env_remove("WAYHARNESS")
        .env_remove("WAYLAND_DISPLAY")
        .output()
        .unwrap()
}

#[test]
fn run_sets_display_and_cleans_up() {
    let output = run(&["--", "sh", "-c", "printf %s \"$WAYLAND_DISPLAY\""]);
    assert!(output.status.success());
    let display = String::from_utf8(output.stdout).unwrap();
    assert!(display.starts_with("wayharness-"), "{display}");
    for suffix in ["", ".lock", ".ctl", ".d"] {
        assert!(!runtime_dir().join(format!("{display}{suffix}")).exists());
    }
}

#[test]
fn run_gives_the_command_a_private_session_bus() {
    let output = run(&["--", "sh", "-c", "printf '%s %s' \"$WAYLAND_DISPLAY\" \"$DBUS_SESSION_BUS_ADDRESS\""]);
    let text = String::from_utf8(output.stdout).unwrap();
    let (display, bus) = text.split_once(' ').unwrap();
    assert_eq!(bus, format!("unix:path={}/{display}.d/bus", runtime_dir().display()));
}

#[test]
fn run_lets_the_command_use_the_server() {
    let output = run(&["--", env!("CARGO_BIN_EXE_wayharness"), "list"]);
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "[]\n");
}

#[test]
fn run_returns_the_exit_status() {
    assert_eq!(run(&["--", "sh", "-c", "exit 3"]).status.code(), Some(3));
}
