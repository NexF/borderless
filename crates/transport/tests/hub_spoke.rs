//! End-to-end test: Hub + 1-2 Spokes inside one process.
//!
//! Validates the v0.2 transport surface:
//!
//! 1. TLS handshake + signed Hello round-trip.
//! 2. TOFU pairing populates each side's [`PeerStore`] with the
//!    other's public-key fingerprint.
//! 3. Strict mode (`accept_new_peers = false`) rejects unknown peers.
//! 4. Frames flow Hub <-> Spoke correctly.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Arc;
use std::time::Duration;

use borderless_clipboard::Engine as ClipEngine;
use borderless_core::{ClipItem, WireFrame};
use borderless_transport::{Connector, Identity, Listener, ListenerConfig, PeerStore};
use parking_lot::Mutex;
use tempfile::tempdir;

fn loopback(port: u16) -> SocketAddr {
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))
}

fn boot(_name: &str, dir: &std::path::Path) -> (Identity, Arc<Mutex<PeerStore>>) {
    let identity = Identity::load_or_generate(dir.join("identity.key")).unwrap();
    let peers = Arc::new(Mutex::new(
        PeerStore::open(dir.join("known_peers.toml")).unwrap(),
    ));
    (identity, peers)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tofu_pair_and_clipboard_round_trip() {
    let hub_dir = tempdir().unwrap();
    let spoke_dir = tempdir().unwrap();

    let (hub_id, hub_peers) = boot("hub", hub_dir.path());
    let (spoke_id, spoke_peers) = boot("spoke", spoke_dir.path());

    let listener = Listener::bind(
        hub_id.clone(),
        hub_peers.clone(),
        ListenerConfig {
            bind: loopback(0),
            name: "hub".into(),
            accept_new_peers: true,
        },
    )
    .await
    .expect("bind hub");
    let hub_addr = listener.local_addr().unwrap();

    let connector = Connector::new(spoke_id.clone(), spoke_peers.clone(), "spoke".into())
        .expect("build connector");

    // Acceptor side runs in its own task.
    let listener = Arc::new(listener);
    let listener_acc = listener.clone();
    let accept_task = tokio::spawn(async move { listener_acc.accept().await });

    let dial = connector
        .dial(hub_addr, None, /*accept_new_peer*/ true)
        .await
        .expect("dial");
    let acc = accept_task.await.unwrap().expect("accept");

    assert_eq!(dial.peer_pubkey, hub_id.pubkey());
    assert_eq!(acc.peer_pubkey, spoke_id.pubkey());

    assert!(hub_peers.lock().is_known(&spoke_id.pubkey()));
    assert!(spoke_peers.lock().is_known(&hub_id.pubkey()));

    // Frame round-trip in both directions.
    let hub_engine = ClipEngine::new(hub_id.node_id(), 8);
    let spoke_engine = ClipEngine::new(spoke_id.node_id(), 8);

    let snap_from_hub = hub_engine.produce_text("hello-from-hub".into());
    acc.send_frame(&WireFrame::Clipboard(snap_from_hub.clone()))
        .await
        .unwrap();
    match tokio::time::timeout(Duration::from_secs(2), dial.recv_frame())
        .await
        .unwrap()
        .unwrap()
    {
        WireFrame::Clipboard(s) => {
            assert_eq!(s.items, vec![ClipItem::Text("hello-from-hub".into())]);
            assert_eq!(s.version, snap_from_hub.version);
        }
        other => panic!("unexpected frame: {other:?}"),
    }

    let snap_from_spoke = spoke_engine.produce_text("hello-from-spoke".into());
    dial.send_frame(&WireFrame::Clipboard(snap_from_spoke.clone()))
        .await
        .unwrap();
    match tokio::time::timeout(Duration::from_secs(2), acc.recv_frame())
        .await
        .unwrap()
        .unwrap()
    {
        WireFrame::Clipboard(s) => {
            assert_eq!(s.items, vec![ClipItem::Text("hello-from-spoke".into())]);
        }
        other => panic!("unexpected frame: {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn strict_mode_rejects_unknown_spoke() {
    let hub_dir = tempdir().unwrap();
    let spoke_dir = tempdir().unwrap();

    let (hub_id, hub_peers) = boot("hub", hub_dir.path());
    let (spoke_id, spoke_peers) = boot("spoke", spoke_dir.path());

    let listener = Listener::bind(
        hub_id.clone(),
        hub_peers.clone(),
        ListenerConfig {
            bind: loopback(0),
            name: "hub".into(),
            accept_new_peers: false,
        },
    )
    .await
    .expect("bind hub");
    let hub_addr = listener.local_addr().unwrap();

    let connector = Connector::new(spoke_id.clone(), spoke_peers.clone(), "spoke".into())
        .expect("build connector");

    let listener = Arc::new(listener);
    let listener_acc = listener.clone();
    let accept_task = tokio::spawn(async move { listener_acc.accept().await });

    let dial_res = connector
        .dial(hub_addr, None, /*accept_new_peer*/ false)
        .await;
    let acc_res = accept_task.await.unwrap();
    assert!(acc_res.is_err(), "hub must reject unknown spoke");
    // The spoke's dial may succeed enough to send Hello, but the
    // recv_frame on its side will see a closed stream once the hub
    // refuses.
    let _ = dial_res;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pubkey_pinning_rejects_wrong_server() {
    let hub_dir = tempdir().unwrap();
    let spoke_dir = tempdir().unwrap();

    let (hub_id, hub_peers) = boot("hub", hub_dir.path());
    let (spoke_id, spoke_peers) = boot("spoke", spoke_dir.path());

    let listener = Listener::bind(
        hub_id.clone(),
        hub_peers.clone(),
        ListenerConfig {
            bind: loopback(0),
            name: "hub".into(),
            accept_new_peers: true,
        },
    )
    .await
    .expect("bind hub");
    let hub_addr = listener.local_addr().unwrap();

    let connector = Connector::new(spoke_id.clone(), spoke_peers.clone(), "spoke".into())
        .expect("build connector");

    let listener = Arc::new(listener);
    let listener_acc = listener.clone();
    let _accept_task = tokio::spawn(async move {
        let _ = listener_acc.accept().await;
    });

    // Pin to a deliberately wrong pubkey.
    let wrong = [0xAB; 32];
    let res = connector
        .dial(hub_addr, Some(wrong), /*accept_new_peer*/ true)
        .await;
    assert!(res.is_err(), "must reject server with wrong pubkey");
}
