use std::{future::Future, pin::Pin};

use atspi::{
    CoordType, Interface,
    proxy::{
        accessible::{AccessibleProxy, ObjectRefExt},
        proxy_ext::ProxyExt,
    },
};
use serde::Serialize;
use zbus::Connection;

use super::failed;

#[derive(Serialize)]
pub struct Extents {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Serialize)]
pub struct Node {
    pub path: String,
    pub role: &'static str,
    pub name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
    pub states: Vec<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extents: Option<Extents>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Node>,
}

pub fn read<'a>(bus: &'a Connection, node: AccessibleProxy<'a>) -> Pin<Box<dyn Future<Output = Result<Node, String>> + 'a>> {
    Box::pin(async move {
        let interfaces = node.get_interfaces().await.map_err(failed)?;
        let proxies = node.proxies().await.map_err(failed)?;
        let extents = match interfaces.contains(Interface::Component) {
            true => {
                let component = proxies.component().await.map_err(failed)?;
                let (x, y, width, height) = component.get_extents(CoordType::Window).await.map_err(failed)?;
                Some(Extents { x, y, width, height })
            }
            false => None,
        };
        let actions = match interfaces.contains(Interface::Action) {
            true => proxies.action().await.map_err(failed)?.get_actions().await.map_err(failed)?,
            false => Vec::new(),
        };
        let value = match interfaces.contains(Interface::Value) {
            true => Some(proxies.value().await.map_err(failed)?.current_value().await.map_err(failed)?),
            false => None,
        };
        let text = match interfaces.contains(Interface::Text) {
            true => {
                let text = proxies.text().await.map_err(failed)?;
                Some(text.get_text(0, -1).await.map_err(failed)?)
            }
            false => None,
        };
        let mut children = Vec::new();
        for child in node.get_children().await.map_err(failed)? {
            if !child.is_null() {
                children.push(read(bus, child.into_accessible_proxy(bus).await.map_err(failed)?).await?);
            }
        }
        Ok(Node {
            path: node.inner().path().to_string(),
            role: node.get_role().await.map_err(failed)?.name(),
            name: node.name().await.map_err(failed)?,
            description: node.description().await.map_err(failed)?,
            states: node.get_state().await.map_err(failed)?.iter().map(|state| state.to_static_str()).collect(),
            extents,
            actions: actions.into_iter().map(|action| action.name).collect(),
            value,
            text,
            children,
        })
    })
}
