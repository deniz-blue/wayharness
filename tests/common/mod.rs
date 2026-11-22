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
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let name = format!("wh-test-{}-{n}", std::process::id());
        let mut child = Command::new(env!("CARGO_BIN_EXE_wayharness"))
            .arg("serve")
            .env("WAYHARNESS", &name)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut ready = String::new();
        BufReader::new(child.stdout.take().unwrap()).read_line(&mut ready).unwrap();
        assert_eq!(ready.trim(), name);
        Self { child, name }
    }

    pub fn ctl(&self, args: &[&str]) -> String {
        let output = Command::new(env!("CARGO_BIN_EXE_wayharness"))
            .args(args)
            .env("WAYHARNESS", &self.name)
            .output()
            .unwrap();
        assert!(output.status.success(), "{args:?}: {}", String::from_utf8_lossy(&output.stderr));
        String::from_utf8(output.stdout).unwrap()
    }

    pub fn screenshot(&self, window: Option<u32>) -> Screenshot {
        let path = std::env::temp_dir().join(format!("{}-{:?}.png", self.name, window));
        let path_str = path.to_str().unwrap();
        match window {
            Some(id) => self.ctl(&["screenshot", "--window", &id.to_string(), path_str]),
            None => self.ctl(&["screenshot", path_str]),
        };
        let decoder = png::Decoder::new(BufReader::new(std::fs::File::open(&path).unwrap()));
        let mut reader = decoder.read_info().unwrap();
        let mut rgba = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut rgba).unwrap();
        std::fs::remove_file(path).ok();
        Screenshot { width: info.width, height: info.height, rgba }
    }

    pub fn wait_for_list(&self, wanted: impl Fn(&str) -> bool) -> String {
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let listing = self.ctl(&["list"]);
            if wanted(&listing) {
                return listing;
            }
            assert!(Instant::now() < deadline, "timed out, last list: {listing:?}");
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

impl Drop for Compositor {
    fn drop(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap();
        for suffix in ["", ".lock", ".ctl"] {
            std::fs::remove_file(format!("{runtime_dir}/{}{suffix}", self.name)).ok();
        }
    }
}
