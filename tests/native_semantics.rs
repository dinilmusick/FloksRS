use floksrs::controls::{Action, Kind, Registry, Widget};
use floksrs::graph_edit::{Editor, Error as EditorError};
use floksrs::music::{Error as MidiError, Geometry, Note, PianoRoll, Section, Timeline};
use floksrs::theme::{Appearance, Direction, Layout, LayoutError, ResolvedAppearance, Theme};
use floksrs::{Edge, GraphNode, Socket, Viewport};

#[test]
fn controls_and_tree_validate_state_and_disposal() {
    let mut registry = Registry::default();
    registry.insert(Widget::new("root", Kind::Label, None)).unwrap();
    registry.insert(Widget::new("field", Kind::TextField, Some("root".into()))).unwrap();
    registry.insert(Widget::new("toggle", Kind::Toggle, Some("root".into()))).unwrap();
    let mut menu = Widget::new("menu", Kind::Menu, Some("root".into()));
    menu.options = vec!["A".into(), "B".into()];
    registry.insert(menu).unwrap();
    assert!(registry.perform("menu", Action::Select("X".into())).is_err());
    registry.perform("menu", Action::Select("B".into())).unwrap();
    registry.perform("field", Action::Input("value".into())).unwrap();
    registry.perform("toggle", Action::Toggle(true)).unwrap();
    assert_eq!(registry.get("field").unwrap().text, "value");
    assert!(registry.get("toggle").unwrap().checked);
    assert!(registry.set_parent("root", Some("menu")).is_err());
    registry.get_mut("root").unwrap().visible = false;
    assert!(registry.perform("field", Action::Input("hidden".into())).is_err());
    registry.get_mut("root").unwrap().visible = true;
    registry.get_mut("field").unwrap().read_only = true;
    assert!(registry.perform("field", Action::Input("locked".into())).is_err());
    assert_eq!(registry.remove_subtree("root").unwrap(), 4);
    assert!(registry.snapshot().is_empty());
    assert!(registry.perform("field", Action::Input("stale".into())).is_err());
}

#[test]
fn graph_history_cycles_selection_and_atomic_moves() {
    let mut editor = Editor::default();
    let mut source = GraphNode::new("source", "midi", 0.0, 0.0);
    source.outputs.push(Socket::new("out", Some("midi")));
    source.inputs.push(Socket::new("in", Some("midi")));
    let mut target = GraphNode::new("target", "synth", 10.0, 10.0);
    target.outputs.push(Socket::new("out", Some("midi")));
    target.inputs.push(Socket::new("in", Some("midi")));
    editor.add_node(source).unwrap();
    editor.add_node(target).unwrap();
    editor.connect_acyclic(Edge {
        id: "connection".into(), source: "source".into(),
        source_socket: "out".into(), target: "target".into(),
        target_socket: "in".into(),
    }).unwrap();
    assert_eq!(editor.connect_acyclic(Edge {
        id: "cycle".into(), source: "target".into(),
        source_socket: "out".into(), target: "source".into(),
        target_socket: "in".into(),
    }), Err(EditorError::Cycle));
    assert!(editor.undo());
    assert!(editor.graph.edges.is_empty());
    assert!(editor.redo());
    assert_eq!(editor.graph.edges.len(), 1);
    let viewport = Viewport { pan_x: 0.0, pan_y: 0.0, zoom: 1.0 };
    assert_eq!(editor.select_rect(-1.0, -1.0, 20.0, 20.0, viewport).unwrap(), 2);
    editor.graph.nodes.get_mut("target").unwrap().position_locked = true;
    assert!(editor.translate_selected(5.0, 0.0).is_err());
    assert_eq!(editor.graph.nodes["source"].x, 0.0);
}

#[test]
fn all_128_midi_rows_round_trip_and_reject_invalid_notes() {
    let geometry = Geometry {
        origin_x: 10.0, origin_y: 20.0,
        pixels_per_tick: 2.0, pitch_height: 8.0,
    };
    for pitch in 0..=127u8 {
        let (x, y) = geometry.note_to_xy(pitch, 123).unwrap();
        assert_eq!(geometry.xy_to_pitch_tick(x, y).unwrap(), (pitch, 123));
    }
    assert_eq!(geometry.note_to_xy(128, 0), Err(MidiError::Pitch));
    assert!(geometry.xy_to_pitch_tick(0.0, 0.0).is_err());
    let next = geometry.zoom_about_x(110.0, 2.0).unwrap();
    assert_eq!(next.note_to_xy(127, 50).unwrap().0, 110.0);
    let mut roll = PianoRoll::default();
    roll.insert(Note {
        id: "first".into(), pitch: 127, start_tick: 8,
        duration_ticks: 4, velocity: 100,
    }).unwrap();
    assert!(roll.move_note("first", 128, 100).is_err());
    assert_eq!(roll.get("first").unwrap().pitch, 127);
    assert!(roll.resize_note("first", 0).is_err());
    assert_eq!(roll.set_loop(Some((20, 20))), Err(MidiError::Loop));
    roll.set_loop(Some((8, 24))).unwrap();
    roll.quantization_ticks = 6;
    assert_eq!(roll.snap(17), 12);
    assert!(roll.insert(Note {
        id: "overflow".into(), pitch: 40,
        start_tick: u64::MAX, duration_ticks: 1, velocity: 50,
    }).is_err());
    assert!(roll.remove("missing").is_err());
}

#[test]
fn appearance_layout_and_sections_stay_pure() {
    let mut theme = Theme {
        appearance: Appearance::System,
        ..Theme::default()
    };
    theme.tokens.insert("accent".into(), "#123456".into());
    assert_eq!(theme.resolve(true), ResolvedAppearance::Dark);
    assert_eq!(theme.resolve(false), ResolvedAppearance::Light);
    assert_eq!(theme.token("accent"), Some("#123456"));
    let layout = Layout {
        direction: Direction::Row, padding: 2, gap: 3, limit: Some(50),
    };
    let rects = layout.place(&[(10, 5), (8, 3)]).unwrap();
    assert_eq!(rects[0].x, 2);
    assert_eq!(rects[1].x, 15);
    assert_eq!(Layout { limit: Some(5), ..layout }.place(&[(10, 5)]),
        Err(LayoutError::ExceedsLimit));
    let mut timeline = Timeline::default();
    timeline.insert(Section {
        id: "intro".into(), label: "Intro".into(),
        start_tick: 0, end_tick: 96,
    }).unwrap();
    assert_eq!(timeline.sections_at(95).len(), 1);
    assert!(timeline.sections_at(96).is_empty());
    assert!(timeline.remove("stale").is_err());
}
