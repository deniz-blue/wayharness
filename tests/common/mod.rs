#![allow(dead_code)]

use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU32, Ordering},
    time::{Duration, Instant},
};

pub mod shm_client;

static COUNTER: AtomicU32 = AtomicU32::new(0);

pub struct Compositor {
    child: Child,
    pub name: String,
    pub session_bus: String,
}

pub struct Client(Child);

impl Drop for Client {
    fn drop(&mut self) {
        self.0.kill().ok();
        self.0.wait().ok();
    }
}

pub fn installed(program: &str) -> bool {
    let found = std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()));
    if !found {
        eprintln!("skipped: {program} is not installed");
    }
    found
}

pub struct Screenshot {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Screenshot {
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 3] {
        let at = ((y * self.width + x) * 4) as usize;
        [self.rgba[at], self.rgba[at + 1], self.rgba[at + 2]]
    }
}

impl Compositor {
    pub fn start() -> Self {
        Self::spawn(&["serve"], Some(&unique_name()))
    }

    pub fn start_with_size(size: &str) -> Self {
        Self::spawn(&["serve", "--size", size], Some(&unique_name()))
    }

    pub fn spawn(args: &[&str], env_display: Option<&str>) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_wayharness"));
        command.args(args).env_remove("WAYHARNESS").env("XKB_DEFAULT_LAYOUT", "us").stdout(Stdio::piped());
        if let Some(display) = env_display {
            command.env("WAYHARNESS", display);
        }
        let mut child = command.spawn().unwrap();
        let mut ready = String::new();
        BufReader::new(child.stdout.take().unwrap()).read_line(&mut ready).unwrap();
        let endpoints: serde_json::Value = serde_json::from_str(&ready).unwrap();
        let name = endpoints["display"].as_str().unwrap().to_string();
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap();
        assert_eq!(endpoints["control"], format!("{runtime_dir}/{name}.ctl"));
        let session_bus = endpoints["session_bus"].as_str().unwrap().to_string();
        Self { child, name, session_bus }
    }

    pub fn try_ctl(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_wayharness")).args(args).env("WAYHARNESS", &self.name).output().unwrap()
    }

    pub fn ctl(&self, args: &[&str]) -> String {
        let output = self.try_ctl(args);
        assert!(output.status.success(), "{args:?}: {}", String::from_utf8_lossy(&output.stderr));
        String::from_utf8(output.stdout).unwrap()
    }

    pub fn start_client(&self, mut command: Command) -> Client {
        command
            .env("WAYLAND_DISPLAY", &self.name)
            .env("DBUS_SESSION_BUS_ADDRESS", &self.session_bus)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        Client(command.spawn().unwrap())
    }

    pub fn screenshot(&self, id: u32) -> Screenshot {
        let path = std::env::temp_dir().join(format!("{}-{id}.png", self.name));
        self.ctl(&["screenshot", &id.to_string(), path.to_str().unwrap()]);
        let decoder = png::Decoder::new(BufReader::new(std::fs::File::open(&path).unwrap()));
        let mut reader = decoder.read_info().unwrap();
        let mut rgba = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut rgba).unwrap();
        std::fs::remove_file(path).ok();
        Screenshot { width: info.width, height: info.height, rgba }
    }

    pub fn windows(&self) -> Vec<serde_json::Value> {
        serde_json::from_str(&self.ctl(&["list"])).unwrap()
    }

    pub fn wait_for_window_size(&self, width: u32, height: u32) {
        let deadline = Instant::now() + Duration::from_secs(60);
        while !self.windows().iter().any(|w| w["width"] == width && w["height"] == height) {
            assert!(Instant::now() < deadline, "no {width}x{height} window, have {:?}", self.windows());
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

fn unique_name() -> String {
    format!("wh-test-{}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::SeqCst))
}

impl Drop for Compositor {
    fn drop(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap();
        for suffix in ["", ".lock", ".ctl"] {
            std::fs::remove_file(format!("{runtime_dir}/{}{suffix}", self.name)).ok();
        }
        std::fs::remove_dir_all(format!("{runtime_dir}/{}.d", self.name)).ok();
    }
}
