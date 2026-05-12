//! mDNS-based peer discovery on the LAN.
//!
//! Service type: `_borderless._udp.local.`
//!
//! TXT properties published:
//!
//! | key | value |
//! |---|---|
//! | `proto` | "0" (protocol version) |
//! | `nodeid` | hex-encoded [`NodeId`] |
//! | `name`   | human-readable host name |
//!
//! The QUIC port is the SRV port; the IP comes from the resolved
//! addresses. Subscribers receive a stream of [`DiscoveredPeer`].

use crate::{Error, Result};
use borderless_core::NodeId;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Service type used by `borderless`.
pub const SERVICE_TYPE: &str = "_borderless._udp.local.";

/// One peer found via mDNS.
#[derive(Clone, Debug)]
pub struct DiscoveredPeer {
    /// Service instance name (typically `<host>._borderless._udp.local.`).
    pub instance: String,
    /// Human-readable name (from TXT `name`).
    pub name: String,
    /// Node id (from TXT `nodeid`); `None` if peer is from another version.
    pub node_id: Option<NodeId>,
    /// Reachable IPs.
    pub addrs: Vec<IpAddr>,
    /// QUIC port.
    pub port: u16,
}

/// Handle to the running mDNS daemon. Drop to shut down.
pub struct DiscoveryHandle {
    daemon: Arc<ServiceDaemon>,
    fullname: String,
}

impl DiscoveryHandle {
    /// Stop advertising.
    pub fn shutdown(self) {
        let _ = self.daemon.unregister(&self.fullname);
        let _ = self.daemon.shutdown();
    }
}

/// Begin advertising this node + start a browse stream of discovered
/// peers. The returned receiver yields all browse events translated
/// into [`DiscoveredPeer`].
pub fn announce_and_browse(
    instance_name: &str,
    node_id: NodeId,
    port: u16,
    advertised_name: &str,
) -> Result<(DiscoveryHandle, mpsc::UnboundedReceiver<DiscoveredPeer>)> {
    let daemon = Arc::new(ServiceDaemon::new().map_err(|e| Error::Discovery(e.to_string()))?);

    let mut props: HashMap<String, String> = HashMap::new();
    props.insert("proto".into(), "0".into());
    props.insert("nodeid".into(), node_id.to_hex());
    props.insert("name".into(), advertised_name.to_string());

    let host_ipv4 = local_ipv4_or_empty();
    let info = ServiceInfo::new(
        SERVICE_TYPE,
        instance_name,
        &format!("{instance_name}.local."),
        host_ipv4.as_str(),
        port,
        Some(props),
    )
    .map_err(|e| Error::Discovery(e.to_string()))?
    .enable_addr_auto();

    let fullname = info.get_fullname().to_string();
    daemon
        .register(info)
        .map_err(|e| Error::Discovery(e.to_string()))?;

    let receiver = daemon
        .browse(SERVICE_TYPE)
        .map_err(|e| Error::Discovery(e.to_string()))?;
    let (tx, rx) = mpsc::unbounded_channel::<DiscoveredPeer>();
    let own_fullname = fullname.clone();

    std::thread::spawn(move || {
        while let Ok(event) = receiver.recv() {
            match event {
                ServiceEvent::ServiceResolved(info) => {
                    if info.get_fullname() == own_fullname {
                        continue;
                    }
                    let props = info.get_properties();
                    let node_id = props
                        .get_property_val_str("nodeid")
                        .and_then(|s| {
                            let bytes = hex::decode(s).ok()?;
                            if bytes.len() != 16 {
                                return None;
                            }
                            let mut id = [0u8; 16];
                            id.copy_from_slice(&bytes);
                            Some(NodeId(id))
                        });
                    let name = props
                        .get_property_val_str("name")
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| info.get_hostname().to_string());

                    let peer = DiscoveredPeer {
                        instance: info.get_fullname().to_string(),
                        name,
                        node_id,
                        addrs: info.get_addresses().iter().copied().collect(),
                        port: info.get_port(),
                    };
                    if tx.send(peer).is_err() {
                        break;
                    }
                }
                ServiceEvent::SearchStopped(_) => break,
                _ => {}
            }
        }
    });

    Ok((
        DiscoveryHandle {
            daemon,
            fullname,
        },
        rx,
    ))
}

fn local_ipv4_or_empty() -> String {
    // `enable_addr_auto` will fill in interfaces; passing "" tells
    // mdns-sd to detect addresses itself.
    String::new()
}
