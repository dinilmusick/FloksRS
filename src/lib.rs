//! Native Floks authoring and graph operations without a React or browser runtime.
//! Contracts: Floks main 5e05c8ce8cadf40b37deeaea42d8b60afe6f0576.
//! Authoring source: packages/canvas-ui/src/authoring/authoringOperations.ts.
//! Graph source: packages/contracts/src/state-graph.ts.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;

use alloc::borrow::ToOwned;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;

pub mod controls;
pub mod graph_edit;
pub mod music;
pub mod theme;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    DuplicateNode(String),
    MissingNode(String),
    MissingParent(String),
    Cycle,
    EmptyBindingProperty,
    DuplicateEdge(String),
    MissingSocket(String),
    IncompatibleSockets,
    InvalidViewport,
    PositionLocked(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
    Service,
    GeneralComponent,
    Application,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Page,
    Section,
    Component,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Text(String),
    Number(f64),
    Boolean(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuthoringNode {
    pub id: String,
    pub kind: NodeKind,
    pub name: String,
    pub component_id: Option<String>,
    pub parent_id: Option<String>,
    pub props: BTreeMap<String, Value>,
    pub slots: BTreeMap<String, Vec<String>>,
    pub states: BTreeMap<String, BTreeMap<String, Value>>,
    pub events: BTreeMap<String, String>,
}

impl AuthoringNode {
    pub fn new(id: impl Into<String>, kind: NodeKind, parent_id: Option<String>) -> Self {
        let name = match kind {
            NodeKind::Page => "Page",
            NodeKind::Section => "Section",
            NodeKind::Component => "Component",
        }
        .to_owned();
        Self {
            id: id.into(),
            kind,
            name,
            component_id: None,
            parent_id,
            props: BTreeMap::new(),
            slots: BTreeMap::new(),
            states: BTreeMap::new(),
            events: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingTarget {
    pub kind: String,
    pub api_id: String,
    pub field: String,
    pub field_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub component_id: String,
    pub property: String,
    pub target: BindingTarget,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuthoringSession {
    pub session_id: String,
    pub context: Context,
    pub nodes: Vec<AuthoringNode>,
    pub bindings: Vec<Binding>,
    pub operations: Vec<String>,
    pub metadata: BTreeMap<String, String>,
}

impl AuthoringSession {
    pub fn new(session_id: impl Into<String>, context: Context) -> Self {
        Self {
            session_id: session_id.into(),
            context,
            nodes: Vec::new(),
            bindings: Vec::new(),
            operations: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }

    pub fn add_node(&mut self, node: AuthoringNode) -> Result<(), Error> {
        if self.nodes.iter().any(|n| n.id == node.id) {
            return Err(Error::DuplicateNode(node.id));
        }
        if let Some(ref parent) = node.parent_id {
            if !self.nodes.iter().any(|n| n.id == *parent) {
                return Err(Error::MissingParent(parent.clone()));
            }
        }
        self.nodes.push(node);
        Ok(())
    }

    pub fn get_children(&self, id: &str) -> Vec<&AuthoringNode> {
        self.nodes
            .iter()
            .filter(|n| n.parent_id.as_deref() == Some(id))
            .collect()
    }

    /// The upstream API includes the requested ID even if it is unknown.
    /// A visited set ensures that malformed cyclic graphs always terminate.
    pub fn subtree_ids(&self, root: &str) -> BTreeSet<String> {
        let mut ids = BTreeSet::from([root.to_owned()]);
        loop {
            let mut changed = false;
            for node in &self.nodes {
                if node.parent_id.as_ref().is_some_and(|id| ids.contains(id)) {
                    changed |= ids.insert(node.id.clone());
                }
            }
            if !changed {
                break;
            }
        }
        ids
    }

    pub fn depth(&self, id: &str) -> Result<usize, Error> {
        let mut parent = self
            .nodes
            .iter()
            .find(|n| n.id == id)
            .ok_or_else(|| Error::MissingNode(id.to_owned()))?
            .parent_id
            .as_deref();
        let mut visited = BTreeSet::from([id.to_owned()]);
        let mut depth = 0;
        while let Some(p) = parent {
            if !visited.insert(p.to_owned()) {
                return Err(Error::Cycle);
            }
            let Some(node) = self.nodes.iter().find(|n| n.id == p) else {
                break;
            };
            depth += 1;
            parent = node.parent_id.as_deref();
        }
        Ok(depth)
    }

    pub fn set_parent(&mut self, id: &str, parent: Option<&str>) -> Result<(), Error> {
        if !self.nodes.iter().any(|n| n.id == id) {
            return Err(Error::MissingNode(id.to_owned()));
        }
        if let Some(new_parent) = parent {
            if !self.nodes.iter().any(|n| n.id == new_parent) {
                return Err(Error::MissingParent(new_parent.to_owned()));
            }
            if self.subtree_ids(id).contains(new_parent) {
                return Err(Error::Cycle);
            }
        }
        self.nodes
            .iter_mut()
            .find(|n| n.id == id)
            .expect("verified node")
            .parent_id = parent.map(ToOwned::to_owned);
        Ok(())
    }

    pub fn remove_node(&mut self, id: &str) -> Result<(), Error> {
        if !self.nodes.iter().any(|n| n.id == id) {
            return Err(Error::MissingNode(id.to_owned()));
        }
        let removed = self.subtree_ids(id);
        self.nodes.retain(|n| !removed.contains(&n.id));
        for node in &mut self.nodes {
            for children in node.slots.values_mut() {
                children.retain(|id| !removed.contains(id));
            }
        }
        self.bindings.retain(|b| !removed.contains(&b.component_id));
        Ok(())
    }

    pub fn bind_property(&mut self, mut binding: Binding) -> Result<(), Error> {
        if !self.nodes.iter().any(|n| n.id == binding.component_id) {
            return Err(Error::MissingNode(binding.component_id));
        }
        binding.property = binding.property.trim().to_owned();
        if binding.property.is_empty() {
            return Err(Error::EmptyBindingProperty);
        }
        self.bindings.retain(|old| {
            old.component_id != binding.component_id || old.property != binding.property
        });
        self.bindings.push(binding);
        Ok(())
    }

    pub fn unbind_property(&mut self, component_id: &str, property: &str) {
        self.bindings
            .retain(|b| b.component_id != component_id || b.property != property);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socket {
    pub id: String,
    pub data_type: Option<String>,
}
impl Socket {
    pub fn new(id: impl Into<String>, data_type: Option<&str>) -> Self {
        Self {
            id: id.into(),
            data_type: data_type.map(ToOwned::to_owned),
        }
    }
}

/// Mirror Floks state-graph's case-insensitive ANY and '*' compatibility.
pub fn compatible(a: &Socket, b: &Socket) -> bool {
    let a = a.data_type.as_deref().unwrap_or("ANY").to_ascii_uppercase();
    let b = b.data_type.as_deref().unwrap_or("ANY").to_ascii_uppercase();
    a.is_empty() || b.is_empty() || a == "ANY" || b == "ANY" || a == "*" || b == "*" || a == b
}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphNode {
    pub id: String,
    pub node_type: String,
    pub x: f64,
    pub y: f64,
    pub position_locked: bool,
    pub inputs: Vec<Socket>,
    pub outputs: Vec<Socket>,
}
impl GraphNode {
    pub fn new(id: impl Into<String>, node_type: impl Into<String>, x: f64, y: f64) -> Self {
        Self {
            id: id.into(),
            node_type: node_type.into(),
            x,
            y,
            position_locked: false,
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub id: String,
    pub source: String,
    pub source_socket: String,
    pub target: String,
    pub target_socket: String,
}
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Graph {
    pub nodes: BTreeMap<String, GraphNode>,
    pub edges: BTreeMap<String, Edge>,
}

impl Graph {
    pub fn add_node(&mut self, node: GraphNode) -> Result<(), Error> {
        if self.nodes.contains_key(&node.id) {
            return Err(Error::DuplicateNode(node.id));
        }
        self.nodes.insert(node.id.clone(), node);
        Ok(())
    }

    pub fn move_node(&mut self, id: &str, x: f64, y: f64) -> Result<(), Error> {
        let node = self
            .nodes
            .get_mut(id)
            .ok_or_else(|| Error::MissingNode(id.into()))?;
        if node.position_locked {
            return Err(Error::PositionLocked(id.into()));
        }
        if !x.is_finite() || !y.is_finite() {
            return Err(Error::InvalidViewport);
        }
        node.x = x;
        node.y = y;
        Ok(())
    }

    pub fn connect(&mut self, edge: Edge) -> Result<(), Error> {
        if self.edges.contains_key(&edge.id) {
            return Err(Error::DuplicateEdge(edge.id));
        }
        let source = self
            .nodes
            .get(&edge.source)
            .ok_or_else(|| Error::MissingNode(edge.source.clone()))?;
        let target = self
            .nodes
            .get(&edge.target)
            .ok_or_else(|| Error::MissingNode(edge.target.clone()))?;
        let output = source
            .outputs
            .iter()
            .find(|s| s.id == edge.source_socket)
            .ok_or_else(|| Error::MissingSocket(edge.source_socket.clone()))?;
        let input = target
            .inputs
            .iter()
            .find(|s| s.id == edge.target_socket)
            .ok_or_else(|| Error::MissingSocket(edge.target_socket.clone()))?;
        if !compatible(output, input) {
            return Err(Error::IncompatibleSockets);
        }
        self.edges.insert(edge.id.clone(), edge);
        Ok(())
    }

    pub fn remove_node(&mut self, id: &str) -> Result<(), Error> {
        if self.nodes.remove(id).is_none() {
            return Err(Error::MissingNode(id.into()));
        }
        self.edges.retain(|_, e| e.source != id && e.target != id);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub pan_x: f64,
    pub pan_y: f64,
    pub zoom: f64,
}
impl Viewport {
    pub fn canvas_to_screen(self, x: f64, y: f64) -> (f64, f64) {
        (x * self.zoom + self.pan_x, y * self.zoom + self.pan_y)
    }

    pub fn zoom_at_point(
        self,
        mx: f64,
        my: f64,
        delta: f64,
        min_zoom: f64,
        max_zoom: f64,
    ) -> Result<Self, Error> {
        if !self.zoom.is_finite()
            || self.zoom <= 0.0
            || !delta.is_finite()
            || !mx.is_finite()
            || !my.is_finite()
            || !min_zoom.is_finite()
            || !max_zoom.is_finite()
            || min_zoom <= 0.0
            || min_zoom > max_zoom
        {
            return Err(Error::InvalidViewport);
        }
        let new_zoom = (self.zoom + delta).clamp(min_zoom, max_zoom);
        let gx = (mx - self.pan_x) / self.zoom;
        let gy = (my - self.pan_y) / self.zoom;
        Ok(Self {
            zoom: new_zoom,
            pan_x: mx - gx * new_zoom,
            pan_y: my - gy * new_zoom,
        })
    }
}
