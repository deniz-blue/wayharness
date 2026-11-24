use atspi::{
    proxy::{
        accessible::{AccessibleProxy, ObjectRefExt},
        bus::{BusProxy, StatusProxy},
    },
};
use zbus::{Connection, fdo::DBusProxy};

async fn connect(address: &str) -> Result<Connection, String> {
    let builder = zbus::connection::Builder::address(address).map_err(|e| format!("{address}: {e}"))?;
    builder.build().await.map_err(|e| format!("{address}: {e}"))
}

pub fn enable_toolkit_support(session_bus: &str) -> Result<(), String> {
    zbus::block_on(async {
        let session = connect(session_bus).await?;
        let status = StatusProxy::new(&session).await.map_err(|e| e.to_string())?;
        status.set_is_enabled(true).await.map_err(|e| e.to_string())
    })
}

pub async fn connect_accessibility_bus(session_bus: &str) -> Result<Connection, String> {
    let session = connect(session_bus).await?;
    let address = BusProxy::new(&session).await.map_err(|e| e.to_string())?;
    let address = address.get_address().await.map_err(|e| format!("no accessibility bus: {e}"))?;
    connect(&address).await
}

pub async fn find_application(bus: &Connection, pid: u32) -> Result<AccessibleProxy<'_>, String> {
    let registry = AccessibleProxy::builder(bus)
        .destination("org.a11y.atspi.Registry")
        .and_then(|builder| builder.path("/org/a11y/atspi/accessible/root"))
        .map_err(|e| e.to_string())?
        .build()
        .await
        .map_err(|e| e.to_string())?;
    let owners = DBusProxy::new(bus).await.map_err(|e| e.to_string())?;
    for application in registry.get_children().await.map_err(|e| e.to_string())? {
        let Some(name) = application.name() else { continue };
        if owners.get_connection_unix_process_id(name.clone().into()).await == Ok(pid) {
            return application.into_accessible_proxy(bus).await.map_err(|e| e.to_string());
        }
    }
    Err(format!("no accessible application for process {pid}"))
}
