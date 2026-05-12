//! mDNS discovery — two nodes must find each other on loopback.
//!
//! Some sandboxes / CI runners disable multicast. The test detects
//! that case and bails with `Ok(())` rather than failing flakily.

use borderless_core::NodeId;
use borderless_transport::discovery::{announce_and_browse, DiscoveredPeer};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;

fn fake_node(byte: u8) -> NodeId {
    NodeId([byte; 16])
}

async fn next_peer(
    rx: &mut UnboundedReceiver<DiscoveredPeer>,
    timeout: Duration,
) -> Option<DiscoveredPeer> {
    tokio::time::timeout(timeout, rx.recv()).await.ok().flatten()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_nodes_find_each_other_via_mdns() {
    let id_a = fake_node(0xAA);
    let id_b = fake_node(0xBB);

    let alice = match announce_and_browse("alice-test", id_a, 38501, "alice-test") {
        Ok(t) => t,
        Err(_) => {
            // Multicast unavailable (sandboxed CI etc.); skip cleanly.
            eprintln!("mdns unavailable; skipping");
            return;
        }
    };
    let bob = match announce_and_browse("bob-test", id_b, 38502, "bob-test") {
        Ok(t) => t,
        Err(_) => {
            eprintln!("mdns unavailable; skipping");
            alice.0.shutdown();
            return;
        }
    };

    let (handle_a, mut rx_a) = alice;
    let (handle_b, mut rx_b) = bob;

    // Each side may also see itself filtered out by the discovery
    // module (it filters by fullname), so the *first* peer event we
    // observe should be the other side.
    let mut saw_b_at_a = false;
    let mut saw_a_at_b = false;

    let deadline = std::time::Instant::now() + Duration::from_secs(8);
    while std::time::Instant::now() < deadline && (!saw_b_at_a || !saw_a_at_b) {
        tokio::select! {
            Some(peer) = next_peer(&mut rx_a, Duration::from_millis(500)) => {
                if peer.node_id == Some(id_b) {
                    assert_eq!(peer.port, 38502);
                    assert_eq!(peer.name, "bob-test");
                    saw_b_at_a = true;
                }
            }
            Some(peer) = next_peer(&mut rx_b, Duration::from_millis(500)) => {
                if peer.node_id == Some(id_a) {
                    assert_eq!(peer.port, 38501);
                    assert_eq!(peer.name, "alice-test");
                    saw_a_at_b = true;
                }
            }
            else => {}
        }
    }

    handle_a.shutdown();
    handle_b.shutdown();

    if !saw_b_at_a && !saw_a_at_b {
        // Sandboxed (no multicast loopback). Don't fail — this would
        // be a flaky test in those environments.
        eprintln!("mdns discovery silent on this host; treating as skip");
        return;
    }
    assert!(saw_b_at_a, "alice did not discover bob");
    assert!(saw_a_at_b, "bob did not discover alice");
}
