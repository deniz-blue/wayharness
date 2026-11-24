mod common;

use std::{
    process::Command,
    time::{Duration, Instant},
};

use common::{Compositor, installed};
use serde_json::Value;

fn find<'a>(node: &'a Value, role: &str, name: &str) -> Option<&'a Value> {
    if node["role"] == role && node["name"] == name {
        return Some(node);
    }
    node["children"].as_array()?.iter().find_map(|child| find(child, role, name))
}

fn start_fixture(compositor: &Compositor) -> common::Client {
    let mut qml = Command::new("qml6");
    qml.arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/accessibility.qml"))
        .env("QT_QPA_PLATFORM", "wayland")
        .env("QT_QUICK_BACKEND", "software");
    compositor.start_client(qml)
}

fn wait_for_tree(compositor: &Compositor) -> Value {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let windows = compositor.windows();
        if let Some(window) = windows.first() {
            let output = compositor.try_ctl(&["accessibility-tree", &window["id"].to_string()]);
            if let Ok(tree) = serde_json::from_slice::<Value>(&output.stdout) {
                if find(&tree, "button", "Press").is_some() {
                    return tree;
                }
            }
        }
        assert!(Instant::now() < deadline, "no accessibility tree, windows: {windows:?}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn path<'a>(tree: &'a Value, role: &str, name: &str) -> &'a str {
    find(tree, role, name).unwrap()["path"].as_str().unwrap()
}

#[test]
fn tree_describes_the_window() {
    if !installed("qml6") {
        return;
    }
    let compositor = Compositor::start();
    let _client = start_fixture(&compositor);
    let tree = wait_for_tree(&compositor);
    assert_eq!(tree["role"], "frame");
    assert_eq!(tree["name"], "accessibility fixture");
    assert_eq!(tree["extents"], serde_json::json!({"x": 0, "y": 0, "width": 320, "height": 240}));
    let button = find(&tree, "button", "Press").unwrap();
    assert!(button["actions"].as_array().unwrap().contains(&"Press".into()));
    assert!(button["states"].as_array().unwrap().contains(&"enabled".into()));
    assert!(button["extents"]["width"].as_i64().unwrap() > 0);
    let field = find(&tree, "text", "field").unwrap();
    assert_eq!(field["description"], "type here");
    assert_eq!(field["text"], "");
}

#[test]
fn set_text_focus_and_press_change_the_window() {
    if !installed("qml6") {
        return;
    }
    let compositor = Compositor::start();
    let _client = start_fixture(&compositor);
    let tree = wait_for_tree(&compositor);
    let field = path(&tree, "text", "field").to_string();
    let button = path(&tree, "button", "Press").to_string();
    let field_text = || {
        let tree: Value = serde_json::from_str(&compositor.ctl(&["accessibility-tree", "1"])).unwrap();
        find(&tree, "text", "field").unwrap()["text"].clone()
    };
    compositor.ctl(&["accessibility-focus", "1", &field]);
    compositor.ctl(&["accessibility-set-text", "1", &field, "typed"]);
    assert_eq!(field_text(), "typed");
    compositor.ctl(&["accessibility-action", "1", &button]);
    assert_eq!(field_text(), "pressed");
    compositor.ctl(&["accessibility-action", "1", &button, "Press"]);
}

#[test]
fn mistakes_name_the_problem() {
    if !installed("qml6") {
        return;
    }
    let compositor = Compositor::start();
    let _client = start_fixture(&compositor);
    let tree = wait_for_tree(&compositor);
    let button = path(&tree, "button", "Press").to_string();
    let error = |args: &[&str]| String::from_utf8(compositor.try_ctl(args).stderr).unwrap();
    assert!(error(&["accessibility-tree", "99"]).contains("no window 99"));
    assert!(error(&["accessibility-action", "1", &button, "Nope"]).contains("has actions"));
    assert!(error(&["accessibility-set-text", "1", &button, "x"]).contains("not editable text"));
    assert!(error(&["accessibility-focus", "1", "/missing"]).contains("no node /missing"));
}
