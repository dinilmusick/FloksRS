use floksrs::*;

fn binding(id: &str, property: &str, field: &str) -> Binding {
    Binding {
        component_id: id.to_owned(), property: property.to_owned(),
        target: BindingTarget { kind: "backend".into(), api_id: "api".into(),
            field: field.into(), field_type: None }
    }
}

#[test]
fn authoring_subtree_removal_cleans_slots_and_bindings() {
    let mut s = AuthoringSession::new("session", Context::Application);
    s.add_node(AuthoringNode::new("page", NodeKind::Page, None)).unwrap();
    s.add_node(AuthoringNode::new("section", NodeKind::Section, Some("page".into()))).unwrap();
    s.add_node(AuthoringNode::new("component", NodeKind::Component, Some("section".into()))).unwrap();
    s.add_node(AuthoringNode::new("other", NodeKind::Component, Some("page".into()))).unwrap();
    s.nodes[0].slots.insert("body".into(), vec!["section".into(), "other".into()]);
    s.bind_property(binding("component", " value ", "one")).unwrap();
    s.bind_property(binding("component", "value", "two")).unwrap();
    s.bind_property(binding("other", "value", "three")).unwrap();
    assert_eq!(s.bindings.len(), 2);
    assert_eq!(s.depth("component").unwrap(), 2);
    assert_eq!(s.get_children("page").len(), 2);
    assert_eq!(s.subtree_ids("section").len(), 2);
    s.remove_node("section").unwrap();
    assert_eq!(s.nodes.len(), 2);
    assert_eq!(s.bindings.len(), 1);
    assert_eq!(s.nodes[0].slots["body"], vec!["other".to_owned()]);
}

#[test]
fn authoring_rejects_duplicate_or_missing_parent_and_cycles() {
    let mut s = AuthoringSession::new("s", Context::Service);
    s.add_node(AuthoringNode::new("a", NodeKind::Page, None)).unwrap();
    assert_eq!(s.add_node(AuthoringNode::new("a", NodeKind::Page, None)),
               Err(Error::DuplicateNode("a".into())));
    assert_eq!(s.add_node(AuthoringNode::new("b", NodeKind::Section, Some("missing".into()))),
               Err(Error::MissingParent("missing".into())));
    s.add_node(AuthoringNode::new("b", NodeKind::Section, Some("a".into()))).unwrap();
    assert_eq!(s.set_parent("a", Some("b")), Err(Error::Cycle));
    assert_eq!(s.set_parent("a", Some("a")), Err(Error::Cycle));
    assert_eq!(s.depth("b").unwrap(), 1);
    assert_eq!(s.bind_property(binding("b", "  ", "f")), Err(Error::EmptyBindingProperty));
    assert_eq!(s.remove_node("absent"), Err(Error::MissingNode("absent".into())));
}

#[test]
fn graph_ports_and_node_removal_behave_like_state_graph() {
    let mut graph = Graph::default();
    let mut source = GraphNode::new("source", "midi", 0.0, 0.0);
    source.outputs.push(Socket::new("out", Some("midi")));
    graph.add_node(source).unwrap();
    let mut target = GraphNode::new("target", "synth", 10.0, 20.0);
    target.inputs.push(Socket::new("in", Some("MIDI")));
    graph.add_node(target).unwrap();
    let edge = Edge { id: "e".into(), source: "source".into(), source_socket: "out".into(),
        target: "target".into(), target_socket: "in".into() };
    graph.connect(edge.clone()).unwrap();
    assert_eq!(graph.connect(edge), Err(Error::DuplicateEdge("e".into())));
    graph.move_node("target", 20.0, 40.0).unwrap();
    graph.nodes.get_mut("target").unwrap().position_locked = true;
    assert_eq!(graph.move_node("target", 1.0, 1.0),
               Err(Error::PositionLocked("target".into())));
    graph.remove_node("source").unwrap();
    assert!(graph.edges.is_empty());
    assert_eq!(graph.nodes.len(), 1);
    assert!(compatible(&Socket::new("a", None), &Socket::new("b", Some("audio"))));
    assert!(compatible(&Socket::new("a", Some("*")), &Socket::new("b", Some("audio"))));
    assert!(!compatible(&Socket::new("a", Some("midi")), &Socket::new("b", Some("audio"))));
}

#[test]
fn viewport_preserves_cursor_world_position_when_zooming() {
    let old = Viewport { pan_x: 100.0, pan_y: 50.0, zoom: 1.0 };
    let new = old.zoom_at_point(200.0, 150.0, 0.5, 0.25, 2.5).unwrap();
    assert_eq!(new.zoom, 1.5);
    assert_eq!(new.canvas_to_screen(100.0, 100.0), (200.0, 150.0));
    assert_eq!(old.zoom_at_point(0.0, 0.0, 10.0, 0.25, 2.5).unwrap().zoom, 2.5);
    assert_eq!(old.zoom_at_point(0.0, 0.0, 1.0, 3.0, 1.0), Err(Error::InvalidViewport));
}
