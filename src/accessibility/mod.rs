mod connection;
mod node;

use atspi::proxy::{
    accessible::{AccessibleProxy, ObjectRefExt},
    proxy_ext::ProxyExt,
};
use clap::Subcommand;
use zbus::{Connection, proxy::CacheProperties};

pub use connection::enable_toolkit_support;

use crate::{client, protocol::Window, session_bus};

#[derive(Subcommand)]
pub enum Command {
    AccessibilityTree { id: u32 },
    AccessibilityAction { id: u32, path: String, name: Option<String> },
    AccessibilitySetText { id: u32, path: String, text: String },
    AccessibilityFocus { id: u32, path: String },
}

impl Command {
    fn window_id(&self) -> u32 {
        match self {
            Command::AccessibilityTree { id }
            | Command::AccessibilityAction { id, .. }
            | Command::AccessibilitySetText { id, .. }
            | Command::AccessibilityFocus { id, .. } => *id,
        }
    }
}

fn failed(error: impl ToString) -> String {
    error.to_string()
}

fn rejected(path: &str) -> impl Fn(zbus::Error) -> String {
    move |error| format!("{path}: {error}")
}

pub fn run(display: Option<String>, command: Command) -> Result<(), String> {
    let display = client::require_display(display)?;
    let id = command.window_id();
    let window = client::windows(&display)?.into_iter().find(|window| window.id == id).ok_or(format!("no window {id}"))?;
    zbus::block_on(async {
        let bus = connection::connect_accessibility_bus(&session_bus::address(&display)).await?;
        let application = connection::find_application(&bus, window.pid).await?;
        match command {
            Command::AccessibilityTree { .. } => {
                let frame = find_frame(&bus, &application, &window).await?;
                println!("{}", serde_json::to_string(&node::read(&bus, frame).await?).map_err(failed)?);
                Ok(())
            }
            Command::AccessibilityAction { path, name, .. } => do_action(&bus, &application, &path, name).await,
            Command::AccessibilitySetText { path, text, .. } => set_text(&bus, &application, &path, &text).await,
            Command::AccessibilityFocus { path, .. } => focus(&bus, &application, &path).await,
        }
    })
}

async fn find_frame<'a>(
    bus: &'a Connection,
    application: &AccessibleProxy<'a>,
    window: &Window,
) -> Result<AccessibleProxy<'a>, String> {
    let mut frames = Vec::new();
    for child in application.get_children().await.map_err(failed)? {
        if !child.is_null() {
            frames.push(child.into_accessible_proxy(bus).await.map_err(failed)?);
        }
    }
    if frames.len() > 1 {
        for index in 0..frames.len() {
            if frames[index].name().await.map_err(failed)? == window.title {
                return Ok(frames.swap_remove(index));
            }
        }
        return Err(format!("no accessible window titled {:?}", window.title));
    }
    frames.pop().ok_or(format!("application of window {} has no accessible window", window.id))
}

async fn node_at<'a>(bus: &'a Connection, application: &AccessibleProxy<'a>, path: &str) -> Result<AccessibleProxy<'a>, String> {
    let node = AccessibleProxy::builder(bus)
        .destination(application.inner().destination().clone())
        .and_then(|builder| builder.path(path.to_owned()))
        .map_err(failed)?
        .cache_properties(CacheProperties::No)
        .build()
        .await
        .map_err(failed)?;
    node.get_role().await.map_err(|_| format!("no node {path}"))?;
    Ok(node)
}

async fn do_action(bus: &Connection, application: &AccessibleProxy<'_>, path: &str, name: Option<String>) -> Result<(), String> {
    let proxies = node_at(bus, application, path).await?.proxies().await.map_err(failed)?;
    let action = proxies.action().await.map_err(|_| format!("{path} has no actions"))?;
    let names: Vec<String> = action.get_actions().await.map_err(failed)?.into_iter().map(|action| action.name).collect();
    let index = match &name {
        Some(name) => names.iter().position(|candidate| candidate == name),
        None => (!names.is_empty()).then_some(0),
    };
    let index = index.ok_or(format!("{path} has actions {names:?}"))?;
    match action.do_action(index as i32).await.map_err(rejected(path))? {
        true => Ok(()),
        false => Err(format!("{path} refused {}", names[index])),
    }
}

async fn set_text(bus: &Connection, application: &AccessibleProxy<'_>, path: &str, text: &str) -> Result<(), String> {
    let proxies = node_at(bus, application, path).await?.proxies().await.map_err(failed)?;
    let editable = proxies.editable_text().await.map_err(|_| format!("{path} is not editable text"))?;
    match editable.set_text_contents(text).await.map_err(rejected(path))? {
        true => Ok(()),
        false => Err(format!("{path} refused the text")),
    }
}

async fn focus(bus: &Connection, application: &AccessibleProxy<'_>, path: &str) -> Result<(), String> {
    let proxies = node_at(bus, application, path).await?.proxies().await.map_err(failed)?;
    let component = proxies.component().await.map_err(|_| format!("{path} cannot take focus"))?;
    match component.grab_focus().await.map_err(rejected(path))? {
        true => Ok(()),
        false => Err(format!("{path} refused focus")),
    }
}
