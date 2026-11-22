//! Reversible graph edits, safe socket connections and selection.
use crate::{Edge, Error as GraphError, Graph, GraphNode, Viewport};
use alloc::borrow::ToOwned;
use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Graph(GraphError),
    Cycle,
    MissingEdge(String),
    InvalidSelection,
}

impl From<GraphError> for Error {
    fn from(value: GraphError) -> Self {
        Self::Graph(value)
    }
}

#[derive(Clone, Debug)]
pub struct Editor {
    pub graph: Graph,
    selected: BTreeSet<String>,
    history: Vec<Graph>,
    index: usize,
}

impl Default for Editor {
    fn default() -> Self {
        let graph = Graph::default();
        Self {
            graph: graph.clone(),
            selected: BTreeSet::new(),
            history: alloc::vec![graph],
            index: 0,
        }
    }
}

impl Editor {
    pub fn selected(&self) -> &BTreeSet<String> {
        &self.selected
    }

    pub fn select(&mut self, id: &str, additive: bool) -> Result<(), Error> {
        if !self.graph.nodes.contains_key(id) {
            return Err(GraphError::MissingNode(id.to_owned()).into());
        }
        if !additive {
            self.selected.clear();
        }
        self.selected.insert(id.to_owned());
        Ok(())
    }

    pub fn clear_selection(&mut self) {
        self.selected.clear();
    }

    pub fn select_rect(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        viewport: Viewport,
    ) -> Result<usize, Error> {
        if !x.is_finite()
            || !y.is_finite()
            || !width.is_finite()
            || !height.is_finite()
            || width < 0.0
            || height < 0.0
            || !viewport.zoom.is_finite()
            || viewport.zoom <= 0.0
        {
            return Err(Error::InvalidSelection);
        }
        self.selected.clear();
        for node in self.graph.nodes.values() {
            let (px, py) = viewport.canvas_to_screen(node.x, node.y);
            if px >= x && py >= y && px <= x + width && py <= y + height {
                self.selected.insert(node.id.clone());
            }
        }
        Ok(self.selected.len())
    }

    fn record(&mut self) {
        self.history.truncate(self.index + 1);
        self.history.push(self.graph.clone());
        self.index += 1;
    }

    pub fn add_node(&mut self, node: GraphNode) -> Result<(), Error> {
        self.graph.add_node(node)?;
        self.record();
        Ok(())
    }

    pub fn remove_node(&mut self, id: &str) -> Result<(), Error> {
        self.graph.remove_node(id)?;
        self.selected.remove(id);
        self.record();
        Ok(())
    }

    pub fn disconnect(&mut self, id: &str) -> Result<(), Error> {
        if self.graph.edges.remove(id).is_none() {
            return Err(Error::MissingEdge(id.to_owned()));
        }
        self.record();
        Ok(())
    }

    fn reachable(&self, start: &str, end: &str) -> bool {
        let mut visited = BTreeSet::new();
        let mut todo = alloc::vec![start.to_owned()];
        while let Some(current) = todo.pop() {
            if current == end {
                return true;
            }
            if !visited.insert(current.clone()) {
                continue;
            }
            for edge in self.graph.edges.values() {
                if edge.source == current {
                    todo.push(edge.target.clone());
                }
            }
        }
        false
    }

    pub fn connect_acyclic(&mut self, edge: Edge) -> Result<(), Error> {
        if edge.source == edge.target || self.reachable(&edge.target, &edge.source) {
            return Err(Error::Cycle);
        }
        self.graph.connect(edge)?;
        self.record();
        Ok(())
    }

    /// Invalid or locked moves do not partially mutate the selected set.
    pub fn translate_selected(&mut self, dx: f64, dy: f64) -> Result<(), Error> {
        if !dx.is_finite() || !dy.is_finite() {
            return Err(Error::InvalidSelection);
        }
        let mut next = self.graph.clone();
        for id in &self.selected {
            let node = next
                .nodes
                .get(id)
                .ok_or_else(|| GraphError::MissingNode(id.clone()))?;
            let (x, y) = (node.x + dx, node.y + dy);
            next.move_node(id, x, y)?;
        }
        if !self.selected.is_empty() {
            self.graph = next;
            self.record();
        }
        Ok(())
    }

    pub fn undo(&mut self) -> bool {
        if self.index == 0 {
            return false;
        }
        self.index -= 1;
        self.graph = self.history[self.index].clone();
        self.selected.retain(|id| self.graph.nodes.contains_key(id));
        true
    }

    pub fn redo(&mut self) -> bool {
        if self.index + 1 >= self.history.len() {
            return false;
        }
        self.index += 1;
        self.graph = self.history[self.index].clone();
        self.selected.retain(|id| self.graph.nodes.contains_key(id));
        true
    }
}
