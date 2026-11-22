mod common;

use std::{
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use common::Compositor;

struct Godot(Child);

impl Drop for Godot {
    fn drop(&mut self) {
        self.0.kill().ok();
        self.0.wait().ok();
    }
}

#[test]
fn screenshots_godot_window() {
    let compositor = Compositor::start();
    let _godot = Godot(
        Command::new("godot")
            .args(["--display-driver", "wayland", "--audio-driver", "Dummy"])
            .args(["--rendering-driver", "opengl3", "--path", concat!(env!("CARGO_MANIFEST_DIR"), "/godot")])
            .env("WAYLAND_DISPLAY", &compositor.name)
            .env("LIBGL_ALWAYS_SOFTWARE", "1")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    compositor.wait_for_list(|list| list.contains("320x240"));
    let deadline = Instant::now() + Duration::from_secs(30);
    let window = loop {
        let window = compositor.screenshot(Some(1));
        if window.pixel(160, 120) == [255, 0, 255] || Instant::now() > deadline {
            break window;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    assert_eq!((window.width, window.height), (320, 240));
    assert_eq!(window.pixel(160, 120), [255, 0, 255]);
}
