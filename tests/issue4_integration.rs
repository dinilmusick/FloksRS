use floksrs::codec::{
    export_authoring, export_graph, export_registry, import_authoring, import_graph,
    import_registry,
};
use floksrs::controls::{Action, Kind, Registry, Widget};
use floksrs::semantic_bridge::Bridge;
use floksrs::{AuthoringNode, Binding, BindingTarget, Context, Graph, GraphNode, NodeKind, Socket};
use frnlrs::semantic::{Data, LifecyclePhase};

#[test]
fn typescript_camel_case_authoring_fixture_and_invalid_handles() {
    // Same camelCase payload format used by Floks UIAuthoringSession.
    let fixture = r#"{"sessionId":"session-one","context":"application","nodes":[{"id":"page","kind":"page","name":"Page","props":{"title":"A","width":100,"visible":true},"slots":{},"states":{},"events":{}}],"bindings":[],"operations":[{"operationId":"execute","name":"Execute","inputFields":[{"name":"input","type":"string"}]}],"metadata":{"origin":"fixture"}}"#;
    let original: serde_json::Value = serde_json::from_str(fixture).unwrap();
    let decoded = import_authoring(fixture).unwrap();
    assert_eq!(decoded.session_id, "session-one");
    assert_eq!(decoded.operations[0].input_fields[0].field_type, "string");
    let encoded: serde_json::Value =
        serde_json::from_str(&export_authoring(&decoded).unwrap()).unwrap();
    assert_eq!(original, encoded);
    let mut cyclic = decoded.clone();
    cyclic.nodes[0].parent_id = Some("page".into());
    assert!(export_authoring(&cyclic).is_err());
    let mut dangling = decoded;
    dangling.bindings.push(Binding {
        component_id: "gone".into(),
        property: "value".into(),
        target: BindingTarget {
            kind: "backend".into(),
            api_id: "api".into(),
            field: "result".into(),
            field_type: None,
        },
    });
    assert!(export_authoring(&dangling).is_err());
    let mut nonfinite = AuthoringNode::new("num", NodeKind::Page, None);
    nonfinite
        .props
        .insert("bad".into(), floksrs::Value::Number(f64::NAN));
    let mut session = floksrs::AuthoringSession::new("another", Context::Application);
    session.add_node(nonfinite).unwrap();
    assert!(export_authoring(&session).is_err());
}

#[test]
fn graph_and_registry_snapshot_roundtrip_and_corruption_rejection() {
    let mut graph = Graph::default();
    let mut first = GraphNode::new("first", "source", 1.0, 2.0);
    first.outputs.push(Socket::new("out", Some("MIDI")));
    let mut second = GraphNode::new("second", "target", 4.0, 6.0);
    second.inputs.push(Socket::new("in", Some("midi")));
    graph.add_node(first).unwrap();
    graph.add_node(second).unwrap();
    graph
        .connect(floksrs::Edge {
            id: "edge".into(),
            source: "first".into(),
            source_socket: "out".into(),
            target: "second".into(),
            target_socket: "in".into(),
        })
        .unwrap();
    let json = export_graph(&graph).unwrap();
    let restored = import_graph(&json).unwrap();
    assert_eq!(restored, graph);
    assert!(import_graph(r#"{"schemaVersion":2,"nodes":[],"edges":[]}"#).is_err());
    let broken = json.replace("\"targetSocket\":\"in\"", "\"targetSocket\":\"gone\"");
    assert!(import_graph(&broken).is_err());
    let mut registry = Registry::default();
    registry
        .insert(Widget::new("page", Kind::Label, None))
        .unwrap();
    registry
        .insert(Widget::new("button", Kind::Button, Some("page".into())))
        .unwrap();
    registry.perform("button", Action::Activate).unwrap();
    let restored = import_registry(&export_registry(&registry).unwrap()).unwrap();
    assert_eq!(restored.snapshot().len(), 2);
    assert_eq!(
        restored.get("button").unwrap().parent.as_deref(),
        Some("page")
    );
    assert!(restored.events.is_empty());
    assert!(import_registry(r#"{"schemaVersion":1,"widgets":[{"id":"x","parent":"missing","kind":"button","label":"","visible":true,"enabled":true,"readOnly":false,"text":"","checked":false,"open":false,"options":[],"selected":null,"metadata":{}}]}"#).is_err());
}

#[test]
fn frnl_semantic_bridge_propagates_state_and_disposes_resources() {
    let mut bridge = Bridge::new();
    bridge
        .mount(Widget::new("page", Kind::Label, None))
        .unwrap();
    bridge
        .mount(Widget::new("play", Kind::Button, Some("page".into())))
        .unwrap();
    bridge
        .mount(Widget::new("name", Kind::TextField, Some("page".into())))
        .unwrap();
    bridge.dispatch("play", Action::Activate).unwrap();
    bridge
        .dispatch("name", Action::Input("value".into()))
        .unwrap();
    assert_eq!(bridge.runtime.history().len(), 2);
    assert_eq!(bridge.runtime.entity("name").unwrap().state, "active");
    assert_eq!(
        bridge.runtime.entity("name").unwrap().data.get("value"),
        Some(&Data::Text("value".into()))
    );
    bridge.suspend("name").unwrap();
    assert!(bridge
        .dispatch("name", Action::Input("blocked".into()))
        .is_err());
    bridge.resume("name").unwrap();
    assert_eq!(
        bridge.runtime.entity("name").unwrap().lifecycle,
        LifecyclePhase::Active
    );
    assert_eq!(bridge.dispose_subtree("page").unwrap(), 3);
    assert!(bridge.runtime.entity("play").is_err());
    assert!(bridge.dispatch("play", Action::Activate).is_err());
}

#[cfg(feature = "framebuffer")]
#[test]
fn framebuffer_writes_actual_pixels_and_rejects_invalid_buffers() {
    use floksrs::framebuffer::Framebuffer;
    use floksrs::theme::{NativeRenderer, Rect, Theme};
    let mut controls = Registry::default();
    let mut button = Widget::new("play", Kind::Button, None);
    button.label = "PLAY".into();
    controls.insert(button).unwrap();
    let mut pixels = vec![0; 50 * 20];
    let mut framebuffer = Framebuffer::new(50, 20, &mut pixels).unwrap();
    framebuffer
        .draw(
            &controls,
            &Theme::default(),
            &[Rect {
                x: 2,
                y: 2,
                width: 40,
                height: 15,
            }],
        )
        .unwrap();
    assert_eq!(framebuffer.pixels()[0], 0x101010);
    assert_eq!(framebuffer.pixels()[2 * 50 + 2], 0x808080);
    assert_ne!(framebuffer.pixels()[5 * 50 + 5], 0x101010);
    assert!(Framebuffer::new(4, 4, &mut [0u32; 3]).is_err());
}

#[cfg(feature = "host-io")]
#[test]
fn midi_port_and_osc_network_packets_have_real_payloads() {
    use floksrs::host_io::{
        send_midi, send_osc, MidiPacket, MidiPort, OscArg, OscMessage, OscPort,
    };
    struct Capture(Vec<u8>);
    impl MidiPort for Capture {
        type Error = ();
        fn send(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
            self.0.extend_from_slice(bytes);
            Ok(())
        }
    }
    impl OscPort for Capture {
        type Error = ();
        fn send(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
            self.0.extend_from_slice(bytes);
            Ok(())
        }
    }
    let midi = MidiPacket::note_on(2, 60, 100).unwrap();
    assert_eq!(midi.as_bytes(), &[0x92, 60, 100]);
    assert!(MidiPacket::note_on(16, 60, 100).is_err());
    assert!(MidiPacket::from_bytes(&[0x92, 255, 4]).is_err());
    let mut port = Capture(Vec::new());
    send_midi(&mut port, &midi).unwrap();
    assert_eq!(&port.0, midi.as_bytes());
    let osc = OscMessage {
        address: "/synth/pitch".into(),
        args: vec![
            OscArg::Int(60),
            OscArg::Float(0.5),
            OscArg::Bool(true),
            OscArg::Text("play".into()),
        ],
    };
    let encoded = osc.encode().unwrap();
    assert_eq!(OscMessage::decode(&encoded).unwrap(), osc);
    assert_eq!(encoded.len() % 4, 0);
    let mut host = Capture(Vec::new());
    send_osc(&mut host, &osc).unwrap();
    assert_eq!(host.0, encoded);
    assert!(OscMessage::decode(&encoded[..encoded.len() - 1]).is_err());
    assert!(OscMessage {
        address: "bad".into(),
        args: Vec::new()
    }
    .encode()
    .is_err());
}
