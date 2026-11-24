mod common;

use std::{
    process::Command,
    time::{Duration, Instant},
};

use common::{Compositor, installed};

#[test]
fn screenshots_godot_window() {
    if !installed("godot") {
        return;
    }
    let compositor = Compositor::start();
    let mut godot = Command::new("godot");
    godot
        .args(["--display-driver", "wayland", "--audio-driver", "Dummy"])
        .args(["--rendering-driver", "opengl3", "--path", concat!(env!("CARGO_MANIFEST_DIR"), "/godot")])
        .env("LIBGL_ALWAYS_SOFTWARE", "1");
    let _godot = compositor.start_client(godot);
    compositor.wait_for_window_size(320, 240);
    let deadline = Instant::now() + Duration::from_secs(30);
    let window = loop {
        let window = compositor.screenshot(1);
        if window.pixel(160, 120) == [255, 0, 255] || Instant::now() > deadline {
            break window;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    assert_eq!((window.width, window.height), (320, 240));
    assert_eq!(window.pixel(160, 120), [255, 0, 255]);
}
