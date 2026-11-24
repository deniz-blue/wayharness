# wayharness

A headless Wayland compositor for tests and agents. Run a GUI app in it, then list its windows, screenshot them, resize them, send input and read the accessibility tree from the command line. Everything prints JSON.

## Install

- `cargo install --git https://github.com/deniz-blue/wayharness`
- Build needs `pixman` and `libxkbcommon` development files
- Accessibility needs `dbus-daemon` and `at-spi2-core` at run time; without them everything else still works

## Start

- `wayharness serve [--size WxH]`: start a compositor on a free `wayharness-N` (size default 1280x720), stop it with Ctrl-C
  - `{"display":"wayharness-0","control":"/run/user/1000/wayharness-0.ctl","session_bus":"unix:path=/run/user/1000/wayharness-0.d/bus"}`
  - Start a client with `WAYLAND_DISPLAY=wayharness-0 DBUS_SESSION_BUS_ADDRESS=<session_bus> CLIENT`
- `wayharness run [--size WxH] -- CMD…`: start one, run CMD with `WAYLAND_DISPLAY`, `DBUS_SESSION_BUS_ADDRESS` and `WAYHARNESS` set, stop it when CMD exits, exit with CMD's status
- Every command below needs `--display NAME` or `WAYHARNESS=NAME`; `serve` also takes them to pick the name
- Errors go to stderr with a nonzero exit

## Windows

- `wayharness list`
  - `[{"app_id":"zenity","height":249,"id":1,"pid":4242,"title":"Entry test","width":300}]`
- `wayharness resize-window ID WIDTH HEIGHT`: ask the window to resize, returns once it has
- `wayharness screenshot ID PATH`: PNG of one window

## Input

- Coordinates are window-relative (X, Y from the window's top left), one seat per server
- `wayharness move-pointer ID X Y`
- `wayharness click ID X Y [--button left|right|middle]`
- `wayharness scroll ID X Y DX DY`: positive DY scrolls down
- `wayharness key ID KEYS`: xkb key names joined with `+`, e.g. `Return`, `Control_L+a`
- `wayharness type ID TEXT`
- Keymap follows `XKB_DEFAULT_LAYOUT`

## Accessibility

- Each server has its own session bus and AT-SPI bus; the tree holds only the apps started in it
- A window is matched to its AT-SPI application by process id
- `wayharness accessibility-tree ID`: the window's tree, nested through `children`
  - `{"path":"/org/a11y/atspi/accessible/2147483650","role":"button","name":"Press","states":["enabled","focusable","sensitive","showing","visible"],"extents":{"x":0,"y":41,"width":80,"height":33},"actions":["Press","Set Focus"]}`
  - `path` identifies the node for the commands below; `description`, `extents`, `actions`, `value` and `text` appear when the node has them
  - `extents` are relative to the window: AT-SPI on Wayland has no screen coordinates
- `wayharness accessibility-action ID PATH [NAME]`: run an action, the first one without NAME
- `wayharness accessibility-set-text ID PATH TEXT`: replace the text of an editable node
- `wayharness accessibility-focus ID PATH`
- Tested with Qt 6 and GTK 4 apps; they register themselves, the server only switches toolkit accessibility on
- What a node reports (names, extents, supported actions) is up to the toolkit; GTK 4 has no focus action

## Limits

- CPU renderer only; clients must use `wl_shm` buffers (no dmabuf, no GPU)
- One fixed output, set at launch
- `xdg-shell` windows only; popups are not rendered or listed
- Screenshots are of one window, not the whole output
- No clipboard or drag and drop; the data device exists so GTK 4 starts
- Linux only

## Tests

- `cargo test`
- Tests that need `godot` or `qml6` skip themselves when it is not installed
