use std::{
    io::{BufRead, BufReader},
    os::unix::{fs::DirBuilderExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};

use crate::{accessibility, control::runtime_dir};

pub struct SessionBus {
    pub address: String,
    dir: PathBuf,
    daemon: Child,
}

pub fn address(display: &str) -> String {
    format!("unix:path={}", directory(display).join("bus").display())
}

fn directory(display: &str) -> PathBuf {
    runtime_dir().join(format!("{display}.d"))
}

fn accessibility_service_file() -> Result<PathBuf, String> {
    let data_dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    data_dirs
        .split(':')
        .map(|dir| Path::new(dir).join("dbus-1/services/org.a11y.Bus.service"))
        .find(|file| file.exists())
        .ok_or("org.a11y.Bus.service not found (is at-spi2-core installed?)".into())
}

fn write_config(dir: &Path, address: &str) -> Result<PathBuf, String> {
    let services = dir.join("services");
    std::fs::create_dir(&services).map_err(|e| format!("{}: {e}", services.display()))?;
    std::fs::copy(accessibility_service_file()?, services.join("org.a11y.Bus.service")).map_err(|e| e.to_string())?;
    let config = dir.join("session.conf");
    let text = format!(
        "<busconfig><type>session</type><listen>{address}</listen><auth>EXTERNAL</auth>\
         <servicedir>{}</servicedir>\
         <policy context=\"default\"><allow send_destination=\"*\" eavesdrop=\"true\"/>\
         <allow eavesdrop=\"true\"/><allow own=\"*\"/></policy></busconfig>",
        services.display()
    );
    std::fs::write(&config, text).map_err(|e| format!("{}: {e}", config.display()))?;
    Ok(config)
}

impl SessionBus {
    pub fn start(display: &str) -> Result<Self, String> {
        let dir = directory(display);
        let bus = Self::launch(display, dir.clone());
        if bus.is_err() {
            std::fs::remove_dir_all(&dir).ok();
        }
        bus
    }

    fn launch(display: &str, dir: PathBuf) -> Result<Self, String> {
        std::fs::remove_dir_all(&dir).ok();
        std::fs::DirBuilder::new().mode(0o700).create(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let address = address(display);
        let config = write_config(&dir, &address)?;
        let mut command = Command::new("dbus-daemon");
        command
            .args(["--nofork", "--print-address=1"])
            .arg(format!("--config-file={}", config.display()))
            .env_remove("DISPLAY")
            .env("XDG_RUNTIME_DIR", &dir)
            .env("WAYLAND_DISPLAY", display)
            .env("DBUS_SESSION_BUS_ADDRESS", &address)
            .env("ATSPI_DBUS_IMPLEMENTATION", "dbus-daemon")
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        unsafe {
            command.pre_exec(|| {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
                Ok(())
            });
        }
        let mut daemon = command.spawn().map_err(|e| format!("dbus-daemon: {e}"))?;
        let mut ready = String::new();
        BufReader::new(daemon.stdout.take().unwrap()).read_line(&mut ready).ok();
        let bus = Self { address, dir, daemon };
        accessibility::enable_toolkit_support(&bus.address)?;
        Ok(bus)
    }
}

impl Drop for SessionBus {
    fn drop(&mut self) {
        self.daemon.kill().ok();
        self.daemon.wait().ok();
        std::fs::remove_dir_all(&self.dir).ok();
    }
}
