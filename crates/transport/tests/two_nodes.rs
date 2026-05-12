//! End-to-end test: two `borderless` nodes inside one process.
//!
//! Validates the v0.1 MVP success criteria without needing real
//! hardware:
//!
//! 1. TOFU pairing populates each side's [`PeerStore`] with the
//!    other's public-key fingerprint.
//! 2. Clipboard snapshots flow A -> B and B -> A, with the
//!    [`Engine`](borderless_clipboard::Engine)'s anti-loop rules
//!    suppressing self-echo.
//! 3. `InputEvent` datagrams ride the unreliable QUIC path and
//!    deserialize correctly on the far side.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Arc;
use std::time::Duration;

use borderless_clipboard::{Decision, Engine as ClipEngine};
use borderless_core::{ClipItem, InputEvent, ModifierMask, NodeId, WireFrame};
use borderless_transport::{Endpoint, EndpointConfig, Identity, PeerStore};
use parking_lot::Mutex;
use tempfile::tempdir;
use tokio::sync::oneshot;

fn loopback(port: u16) -> SocketAddr {
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))
}

async fn boot(name: &str, dir: &std::path::Path) -> (Identity, Arc<Endpoint>) {
    let identity = Identity::load_or_generate(dir.join("identity.key")).unwrap();
    let peers = Arc::new(Mutex::new(
        PeerStore::open(dir.join("known_peers.toml")).unwrap(),
    ));
    let cfg = EndpointConfig {
        bind: loopback(0),
        name: name.into(),
        allow_new_peers: true,
    };
    let endpoint = Endpoint::bind(identity.clone(), peers, cfg).expect("bind");
    (identity, Arc::new(endpoint))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pairing_and_clipboard_sync() {
    let dir_a = tempdir().unwrap();
    let dir_b = tempdir().unwrap();

    let (id_a, ep_a) = boot("alice", dir_a.path()).await;
    let (id_b, ep_b) = boot("bob", dir_b.path()).await;

    let addr_b = ep_b.local_addr().unwrap();

    // Acceptor side: spawn the accept future first.
    let ep_b_acceptor = ep_b.clone();
    let (accept_tx, accept_rx) = oneshot::channel();
    tokio::spawn(async move {
        let conn = ep_b_acceptor.accept().await.expect("accept");
        let _ = accept_tx.send(conn);
    });

    // Initiator dials.
    let conn_a = tokio::time::timeout(Duration::from_secs(5), ep_a.connect(addr_b))
        .await
        .expect("connect timely")
        .expect("connect ok");
    let conn_b = tokio::time::timeout(Duration::from_secs(5), accept_rx)
        .await
        .expect("accept timely")
        .expect("accept ok");

    assert_eq!(conn_a.peer_node_id, id_b.node_id());
    assert_eq!(conn_b.peer_node_id, id_a.node_id());
    assert_eq!(conn_a.peer_name, "bob");
    assert_eq!(conn_b.peer_name, "alice");

    // Both peer stores must now know about the other.
    let peers_a = PeerStore::open(dir_a.path().join("known_peers.toml")).unwrap();
    let peers_b = PeerStore::open(dir_b.path().join("known_peers.toml")).unwrap();
    assert!(peers_a.is_known(&id_b.pubkey()), "alice should know bob");
    assert!(peers_b.is_known(&id_a.pubkey()), "bob should know alice");

    // ----- Clipboard sync, both directions, with anti-loop. -----
    let clip_a = ClipEngine::new(id_a.node_id(), 16);
    let clip_b = ClipEngine::new(id_b.node_id(), 16);

    // A produces "from-alice", broadcasts to B.
    let snap_a = clip_a.produce_text("from-alice".into());
    conn_a
        .send_frame(&WireFrame::Clipboard(snap_a.clone()))
        .await
        .unwrap();
    let received = conn_b.recv_frame().await.unwrap();
    let received_snap = match received {
        WireFrame::Clipboard(s) => s,
        other => panic!("expected clipboard, got {other:?}"),
    };
    assert_eq!(received_snap, snap_a);
    match clip_b.observe_remote(received_snap.clone()) {
        Decision::Apply(s) => assert_eq!(s.items, vec![ClipItem::Text("from-alice".into())]),
        Decision::Ignore => panic!("B should accept fresh A snapshot"),
    }

    // B echoes back: re-broadcasting the same snapshot must NOT
    // create a loop on A.
    conn_b
        .send_frame(&WireFrame::Clipboard(received_snap.clone()))
        .await
        .unwrap();
    let echoed = match conn_a.recv_frame().await.unwrap() {
        WireFrame::Clipboard(s) => s,
        other => panic!("expected clipboard, got {other:?}"),
    };
    assert_eq!(
        clip_a.observe_remote(echoed),
        Decision::Ignore,
        "A must drop its own snapshot"
    );

    // B produces a new one, A accepts and advances clock.
    let snap_b = clip_b.produce_text("from-bob".into());
    conn_b
        .send_frame(&WireFrame::Clipboard(snap_b.clone()))
        .await
        .unwrap();
    let received_at_a = match conn_a.recv_frame().await.unwrap() {
        WireFrame::Clipboard(s) => s,
        other => panic!("expected clipboard, got {other:?}"),
    };
    match clip_a.observe_remote(received_at_a) {
        Decision::Apply(s) => assert_eq!(s.items, vec![ClipItem::Text("from-bob".into())]),
        Decision::Ignore => panic!("A should accept B's later snapshot"),
    }

    // ----- Input datagram path. -----
    let mouse = InputEvent::MouseMove {
        dx: 10,
        dy: -3,
        ts: 12345,
    };
    conn_a.send_datagram(&mouse).unwrap();
    let received = tokio::time::timeout(Duration::from_secs(2), conn_b.recv_datagram())
        .await
        .expect("datagram timely")
        .expect("datagram ok");
    assert_eq!(received, mouse);

    // ----- Sanity: a Key event also serializes through datagrams. -----
    let key = InputEvent::Key {
        code: borderless_core::HidUsage::KEY_A,
        pressed: true,
        modifiers: ModifierMask::LSHIFT,
    };
    conn_a.send_datagram(&key).unwrap();
    let echoed = tokio::time::timeout(Duration::from_secs(2), conn_b.recv_datagram())
        .await
        .expect("datagram timely")
        .expect("datagram ok");
    assert_eq!(echoed, key);

    conn_a.close();
    conn_b.close();
    let _ = id_a;
    let _ = id_b;
    let _ = NodeId::from_pubkey(&[0; 32]); // satisfy the import linter
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reconnect_after_close_with_known_peer_works_in_strict_mode() {
    // First session: pair via TOFU (allow_new_peers=true on both).
    // Second session: both sides set allow_new_peers=false; the
    // reconnect must still succeed because the peer fingerprint was
    // persisted to known_peers.toml.
    let dir_a = tempdir().unwrap();
    let dir_b = tempdir().unwrap();

    {
        let (_id_a, ep_a) = boot("alice", dir_a.path()).await;
        let (_id_b, ep_b) = boot("bob", dir_b.path()).await;
        let addr_b = ep_b.local_addr().unwrap();
        let acc = tokio::spawn(async move { ep_b.accept().await.unwrap() });
        let conn_a = ep_a.connect(addr_b).await.unwrap();
        let conn_b = acc.await.unwrap();
        conn_a.close();
        conn_b.close();
        ep_a.close().await;
    }

    // Strict mode for round 2.
    let id_a = Identity::load_or_generate(dir_a.path().join("identity.key")).unwrap();
    let id_b = Identity::load_or_generate(dir_b.path().join("identity.key")).unwrap();
    let peers_a = Arc::new(Mutex::new(
        PeerStore::open(dir_a.path().join("known_peers.toml")).unwrap(),
    ));
    let peers_b = Arc::new(Mutex::new(
        PeerStore::open(dir_b.path().join("known_peers.toml")).unwrap(),
    ));
    assert!(peers_a.lock().is_known(&id_b.pubkey()));
    assert!(peers_b.lock().is_known(&id_a.pubkey()));

    let ep_a = Arc::new(
        Endpoint::bind(
            id_a.clone(),
            peers_a,
            EndpointConfig {
                bind: loopback(0),
                name: "alice".into(),
                allow_new_peers: false,
            },
        )
        .unwrap(),
    );
    let ep_b = Arc::new(
        Endpoint::bind(
            id_b.clone(),
            peers_b,
            EndpointConfig {
                bind: loopback(0),
                name: "bob".into(),
                allow_new_peers: false,
            },
        )
        .unwrap(),
    );
    let addr_b = ep_b.local_addr().unwrap();
    let ep_b_acc = ep_b.clone();
    let acc = tokio::spawn(async move { ep_b_acc.accept().await });
    let conn_a = ep_a.connect(addr_b).await.expect("strict reconnect");
    let conn_b = tokio::time::timeout(Duration::from_secs(3), acc)
        .await
        .unwrap()
        .unwrap()
        .expect("strict accept");
    assert_eq!(conn_a.peer_node_id, id_b.node_id());
    assert_eq!(conn_b.peer_node_id, id_a.node_id());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn many_clipboard_frames_in_a_row_preserve_order() {
    let dir_a = tempdir().unwrap();
    let dir_b = tempdir().unwrap();
    let (id_a, ep_a) = boot("alice", dir_a.path()).await;
    let (_id_b, ep_b) = boot("bob", dir_b.path()).await;

    let addr_b = ep_b.local_addr().unwrap();
    let ep_b_acc = ep_b.clone();
    let (tx, rx) = oneshot::channel();
    tokio::spawn(async move {
        let conn = ep_b_acc.accept().await.unwrap();
        let _ = tx.send(conn);
    });
    let conn_a = ep_a.connect(addr_b).await.unwrap();
    let conn_b = rx.await.unwrap();

    let clip_a = ClipEngine::new(id_a.node_id(), 16);
    let mut sent = Vec::new();
    for i in 0..16 {
        let snap = clip_a.produce_text(format!("v{i}"));
        sent.push(snap.clone());
        conn_a
            .send_frame(&WireFrame::Clipboard(snap))
            .await
            .unwrap();
    }
    for expected in sent {
        let frame = tokio::time::timeout(Duration::from_secs(2), conn_b.recv_frame())
            .await
            .unwrap()
            .unwrap();
        match frame {
            WireFrame::Clipboard(snap) => assert_eq!(snap, expected),
            other => panic!("expected clipboard, got {other:?}"),
        }
    }
    conn_a.close();
    conn_b.close();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unknown_peer_is_rejected_outside_pairing_mode() {
    let dir_a = tempdir().unwrap();
    let dir_b = tempdir().unwrap();

    // A is in *strict* mode (allow_new_peers=false). B will dial.
    let id_a = Identity::load_or_generate(dir_a.path().join("identity.key")).unwrap();
    let peers_a = Arc::new(Mutex::new(
        PeerStore::open(dir_a.path().join("known_peers.toml")).unwrap(),
    ));
    let ep_a = Arc::new(
        Endpoint::bind(
            id_a,
            peers_a,
            EndpointConfig {
                bind: loopback(0),
                name: "alice".into(),
                allow_new_peers: false,
            },
        )
        .unwrap(),
    );
    let addr_a = ep_a.local_addr().unwrap();

    let id_b = Identity::load_or_generate(dir_b.path().join("identity.key")).unwrap();
    let peers_b = Arc::new(Mutex::new(
        PeerStore::open(dir_b.path().join("known_peers.toml")).unwrap(),
    ));
    let ep_b = Endpoint::bind(
        id_b,
        peers_b,
        EndpointConfig {
            bind: loopback(0),
            name: "bob".into(),
            allow_new_peers: true,
        },
    )
    .unwrap();

    let ep_a_acceptor = ep_a.clone();
    let acc = tokio::spawn(async move { ep_a_acceptor.accept().await });
    let dial = ep_b.connect(addr_a).await;

    // Both sides should fail because A rejects an unknown pubkey.
    let acc_res = tokio::time::timeout(Duration::from_secs(3), acc)
        .await
        .unwrap()
        .unwrap();
    assert!(acc_res.is_err(), "alice must reject unknown peer");
    assert!(dial.is_err(), "bob's dial must fail because alice closes");
}
