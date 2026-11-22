# wayharness

- Start: `cargo run -- serve` (prints the socket name, `wayharness` by default; set `WAYHARNESS=name` for another one)
- Run a client in it: `WAYLAND_DISPLAY=wayharness some-client`
- Every command below needs the same `WAYHARNESS` as the compositor
- `wayharness list`: one line per window, `id<TAB>app_id<TAB>title<TAB>WxH`
- `wayharness resize-output W H`: change the virtual output size (default 1280x720)
- `wayharness resize-window ID W H`: ask the window to resize, returns once it has
- `wayharness screenshot PATH`: PNG of the whole output
- `wayharness screenshot --window ID PATH`: PNG of one window
- Errors go to stderr with a nonzero exit
- Tests: `cargo test` (needs `godot` on PATH for the Godot one)
