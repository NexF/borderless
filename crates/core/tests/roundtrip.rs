use borderless_core::{
    decode, encode, Button, ClipItem, ClipboardSnapshot, ControlFrame, HidUsage, InputEvent,
    ModifierMask, NodeId, ProtocolVersion, WireFrame, PROTOCOL_V0,
};

fn sample_node() -> NodeId {
    NodeId::from_pubkey(&[7u8; 32])
}

#[test]
fn input_event_roundtrips() {
    let cases = [
        InputEvent::MouseMove {
            dx: -3,
            dy: 5,
            ts: 12_345,
        },
        InputEvent::MouseButton {
            btn: Button::Left,
            pressed: true,
        },
        InputEvent::Scroll { dx: 0, dy: -120 },
        InputEvent::Key {
            code: HidUsage::KEY_A,
            pressed: true,
            modifiers: ModifierMask::LSHIFT | ModifierMask::LCTRL,
        },
        InputEvent::Enter {
            from: sample_node(),
            modifiers: ModifierMask::empty(),
        },
        InputEvent::Leave { to: sample_node() },
    ];

    for ev in cases {
        let bytes = encode(&ev).expect("encode");
        let back: InputEvent = decode(&bytes).expect("decode");
        assert_eq!(ev, back);
    }
}

#[test]
fn clipboard_snapshot_roundtrips() {
    let snap = ClipboardSnapshot {
        version: 42,
        origin: sample_node(),
        created_unix_ms: 1_700_000_000_000,
        items: vec![ClipItem::Text("hello".into())],
    };

    let bytes = encode(&snap).unwrap();
    let back: ClipboardSnapshot = decode(&bytes).unwrap();
    assert_eq!(snap, back);
}

#[test]
fn wire_frame_envelope_roundtrips() {
    let hello = WireFrame::Control(ControlFrame::Hello {
        node_id: sample_node(),
        name: "alice".into(),
        max_protocol: PROTOCOL_V0,
    });

    let bytes = encode(&hello).unwrap();
    let back: WireFrame = decode(&bytes).unwrap();
    assert_eq!(hello, back);
}

#[test]
fn protocol_version_ordering() {
    assert!(PROTOCOL_V0 < ProtocolVersion(1));
    assert!(ProtocolVersion(2) > ProtocolVersion(1));
}

#[test]
fn node_id_is_deterministic_per_pubkey() {
    let pk = [9u8; 32];
    assert_eq!(NodeId::from_pubkey(&pk), NodeId::from_pubkey(&pk));
    assert_ne!(NodeId::from_pubkey(&pk), NodeId::from_pubkey(&[0u8; 32]));
}
