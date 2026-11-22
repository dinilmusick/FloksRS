//! Versioned canonical-compatible JSON snapshots. No host handles are serialized.
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
use crate::{AuthoringSession, Edge, Graph, GraphNode, Socket};
use crate::controls::{Registry, Widget};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    Syntax(String),
    Schema(String),
    Invalid(String),
}
fn syntax(error: serde_json::Error) -> CodecError { CodecError::Syntax(error.to_string()) }
fn invalid(value: impl Into<String>) -> CodecError { CodecError::Invalid(value.into()) }
fn required(id: &str) -> Result<(), CodecError> {
    if id.trim().is_empty() { Err(invalid("identifier cannot be empty")) } else { Ok(()) }
}

pub fn validate_session(session: &AuthoringSession) -> Result<(), CodecError> {
    required(&session.session_id)?;
    let mut nodes = BTreeMap::new();
    for node in &session.nodes {
        required(&node.id)?;
        if nodes.insert(node.id.as_str(), node).is_some() {
            return Err(invalid(format!("duplicate authoring node {}", node.id)));
        }
        for value in node.props.values() {
            if let crate::Value::Number(x) = value {
                if !x.is_finite() { return Err(invalid("non-finite property")); }
            }
        }
        for state in node.states.values() {
            for value in state.values() {
                if let crate::Value::Number(x) = value {
                    if !x.is_finite() { return Err(invalid("non-finite state")); }
                }
            }
        }
    }
    for node in &session.nodes {
        let mut parents = BTreeSet::new();
        let mut parent = node.parent_id.as_deref();
        parents.insert(node.id.as_str());
        while let Some(id) = parent {
            if !parents.insert(id) { return Err(invalid("authoring parent cycle")); }
            let next = nodes.get(id).ok_or_else(|| invalid(format!("unknown parent {id}")))?;
            parent = next.parent_id.as_deref();
        }
        for children in node.slots.values() {
            for child in children {
                if !nodes.contains_key(child.as_str()) {
                    return Err(invalid(format!("dangling slot child {child}")));
                }
            }
        }
    }
    let mut bound = BTreeSet::new();
    for binding in &session.bindings {
        if !nodes.contains_key(binding.component_id.as_str()) {
            return Err(invalid(format!("dangling binding {}", binding.component_id)));
        }
        required(&binding.property)?;
        if !bound.insert((&binding.component_id, &binding.property)) {
            return Err(invalid("duplicate property binding"));
        }
    }
    Ok(())
}

/// Uses Floks camelCase authoring keys; rejects stale/cyclic references.
pub fn export_authoring(session: &AuthoringSession) -> Result<String, CodecError> {
    validate_session(session)?;
    serde_json::to_string(session).map_err(syntax)
}
pub fn import_authoring(source: &str) -> Result<AuthoringSession, CodecError> {
    let doc: AuthoringSession = serde_json::from_str(source).map_err(syntax)?;
    validate_session(&doc)?;
    Ok(doc)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct Position { x: f64, y: f64 }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
struct WireSocket {
    id: String,
    #[serde(default)] label: String,
    #[serde(rename="type")] direction: String,
    #[serde(default, skip_serializing_if="Option::is_none")] data_type: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
struct WireNode {
    id: String,
    #[serde(rename="type")] node_type: String,
    #[serde(default)] label: String,
    position: Position,
    #[serde(default)] position_locked: bool,
    #[serde(default)] inputs: Vec<WireSocket>,
    #[serde(default)] outputs: Vec<WireSocket>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
struct WireEdge {
    id: String,
    source: String,
    source_socket: String,
    target: String,
    target_socket: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct GraphEnvelope { schema_version: u32, nodes: Vec<WireNode>, edges: Vec<WireEdge> }

fn socket_to_wire(s: &Socket, direction: &str) -> WireSocket {
    WireSocket { id: s.id.clone(), label: s.id.clone(),
        direction: direction.to_string(), data_type: s.data_type.clone() }
}
pub fn export_graph(graph: &Graph) -> Result<String, CodecError> {
    for node in graph.nodes.values() {
        if !node.x.is_finite() || !node.y.is_finite() { return Err(invalid("invalid node coordinate")); }
    }
    let wire = GraphEnvelope {
        schema_version: SCHEMA_VERSION,
        nodes: graph.nodes.values().map(|n| WireNode {
            id: n.id.clone(), node_type: n.node_type.clone(), label: n.id.clone(),
            position: Position { x: n.x, y: n.y }, position_locked: n.position_locked,
            inputs: n.inputs.iter().map(|s| socket_to_wire(s, "input")).collect(),
            outputs: n.outputs.iter().map(|s| socket_to_wire(s, "output")).collect()
        }).collect(),
        edges: graph.edges.values().map(|e| WireEdge {
            id:e.id.clone(), source:e.source.clone(), source_socket:e.source_socket.clone(),
            target:e.target.clone(), target_socket:e.target_socket.clone()
        }).collect()
    };
    // Re-run semantic validation against our own document to catch direct public graph mutation.
    let encoded = serde_json::to_string(&wire).map_err(syntax)?;
    import_graph(&encoded)?;
    Ok(encoded)
}
pub fn import_graph(source: &str) -> Result<Graph, CodecError> {
    let wire: GraphEnvelope = serde_json::from_str(source).map_err(syntax)?;
    if wire.schema_version != SCHEMA_VERSION {
        return Err(CodecError::Schema("unsupported graph schema version".into()));
    }
    let mut graph = Graph::default();
    for n in wire.nodes {
        required(&n.id)?;
        required(&n.node_type)?;
        if !n.position.x.is_finite() || !n.position.y.is_finite() {
            return Err(invalid("non-finite node coordinate"));
        }
        let mut seen = BTreeSet::new();
        let mut node = GraphNode::new(n.id.clone(), n.node_type, n.position.x, n.position.y);
        node.position_locked = n.position_locked;
        for s in n.inputs {
            required(&s.id)?;
            if s.direction != "input" || !seen.insert((0u8, s.id.clone())) {
                return Err(invalid("invalid or duplicate input socket"));
            }
            node.inputs.push(Socket { id:s.id, data_type:s.data_type });
        }
        for s in n.outputs {
            required(&s.id)?;
            if s.direction != "output" || !seen.insert((1u8, s.id.clone())) {
                return Err(invalid("invalid or duplicate output socket"));
            }
            node.outputs.push(Socket { id:s.id, data_type:s.data_type });
        }
        graph.add_node(node).map_err(|e| invalid(format!("{e:?}")))?;
    }
    for e in wire.edges {
        required(&e.id)?;
        graph.connect(Edge { id:e.id, source:e.source, source_socket:e.source_socket,
            target:e.target, target_socket:e.target_socket })
            .map_err(|e| invalid(format!("{e:?}")))?;
    }
    Ok(graph)
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct RegistryEnvelope {
    schema_version: u32,
    widgets: Vec<Widget>,
}
pub fn export_registry(registry: &Registry) -> Result<String, CodecError> {
    let wire = RegistryEnvelope {
        schema_version: SCHEMA_VERSION,
        widgets: registry.snapshot().into_iter().cloned().collect(),
    };
    let json = serde_json::to_string(&wire).map_err(syntax)?;
    import_registry(&json)?;
    Ok(json)
}
pub fn import_registry(source: &str) -> Result<Registry, CodecError> {
    let wire: RegistryEnvelope = serde_json::from_str(source).map_err(syntax)?;
    if wire.schema_version != SCHEMA_VERSION {
        return Err(CodecError::Schema("unsupported registry schema version".into()));
    }
    let mut registry = Registry::default();
    let mut waiting = wire.widgets;
    let mut ids = BTreeSet::new();
    for w in &waiting {
        required(&w.id)?;
        if !ids.insert(w.id.clone()) { return Err(invalid("duplicate widget identifier")); }
        if let Some(selected) = &w.selected {
            if !w.options.contains(selected) { return Err(invalid("stale widget selection")); }
        }
    }
    while !waiting.is_empty() {
        let before = waiting.len();
        let mut remainder = Vec::new();
        for widget in waiting {
            let ready = widget.parent.as_ref().is_none_or(|p| registry.get(p).is_some());
            if ready {
                registry.insert(widget).map_err(|e| invalid(format!("{e:?}")))?;
            } else {
                remainder.push(widget);
            }
        }
        if remainder.len() == before { return Err(invalid("dangling parent or widget cycle")); }
        waiting = remainder;
    }
    Ok(registry)
}
