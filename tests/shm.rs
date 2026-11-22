mod common;

use common::{Compositor, shm_client};

const COLOUR: [u8; 3] = [0x20, 0x80, 0xe0];

fn compositor_with_window() -> Compositor {
    let compositor = Compositor::start();
    shm_client::spawn(&compositor.name, COLOUR);
    compositor.wait_for_list(|list| list.contains("64x48"));
    compositor
}

#[test]
fn lists_window() {
    let compositor = compositor_with_window();
    assert_eq!(compositor.ctl(&["list"]), "1\twh-test\thello\t64x48\n");
}

#[test]
fn screenshots_output_and_window() {
    let compositor = compositor_with_window();
    let output = compositor.screenshot(None);
    assert_eq!((output.width, output.height), (1280, 720));
    assert_eq!(output.pixel(10, 10), COLOUR);
    assert_eq!(output.pixel(1000, 600), [25, 25, 25]);
    let window = compositor.screenshot(Some(1));
    assert_eq!((window.width, window.height), (64, 48));
    assert_eq!(window.pixel(32, 24), COLOUR);
}

#[test]
fn resizes_window_and_output() {
    let compositor = compositor_with_window();
    compositor.ctl(&["resize-window", "1", "100", "80"]);
    let window = compositor.screenshot(Some(1));
    assert_eq!((window.width, window.height), (100, 80));
    assert_eq!(window.pixel(99, 79), COLOUR);
    compositor.ctl(&["resize-output", "800", "600"]);
    let output = compositor.screenshot(None);
    assert_eq!((output.width, output.height), (800, 600));
}
