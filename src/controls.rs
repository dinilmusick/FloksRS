//! Renderer-neutral UI controls, authoring state, lifecycle and real host bindings.
use alloc::borrow::ToOwned;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;
use frnlrs::{Control, Descriptor, Surface, SurfaceError, Value};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Button,
    TextField,
    Toggle,
    Tile,
    Menu,
    Selector,
    Tabs,
    Dialog,
    Label,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Widget {
    pub id: String,
    pub parent: Option<String>,
    pub kind: Kind,
    pub label: String,
    pub visible: bool,
    pub enabled: bool,
    pub read_only: bool,
    pub text: String,
    pub checked: bool,
    pub open: bool,
    pub options: Vec<String>,
    pub selected: Option<String>,
    pub metadata: BTreeMap<String, String>,
}

impl Widget {
    pub fn new(id: impl Into<String>, kind: Kind, parent: Option<String>) -> Self {
        Self {
            id: id.into(),
            parent,
            kind,
            label: String::new(),
            visible: true,
            enabled: true,
            read_only: false,
            text: String::new(),
            checked: false,
            open: false,
            options: Vec::new(),
            selected: None,
            metadata: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    EmptyId,
    Duplicate(String),
    Missing(String),
    Cycle,
    Unavailable(String),
    ReadOnly(String),
    InvalidAction,
    InvalidOption(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Activate,
    Input(String),
    Select(String),
    Toggle(bool),
    Show,
    Hide,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub widget_id: String,
    pub action: Action,
}

#[derive(Clone, Debug, Default)]
pub struct Registry {
    widgets: BTreeMap<String, Widget>,
    pub events: Vec<Event>,
}

impl Registry {
    pub fn insert(&mut self, widget: Widget) -> Result<(), Error> {
        if widget.id.trim().is_empty() {
            return Err(Error::EmptyId);
        }
        if self.widgets.contains_key(&widget.id) {
            return Err(Error::Duplicate(widget.id));
        }
        if let Some(parent) = &widget.parent {
            if !self.widgets.contains_key(parent) {
                return Err(Error::Missing(parent.clone()));
            }
        }
        self.widgets.insert(widget.id.clone(), widget);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&Widget> {
        self.widgets.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Widget> {
        self.widgets.get_mut(id)
    }

    pub fn snapshot(&self) -> Vec<&Widget> {
        self.widgets.values().collect()
    }

    pub fn set_parent(&mut self, id: &str, parent: Option<&str>) -> Result<(), Error> {
        if !self.widgets.contains_key(id) {
            return Err(Error::Missing(id.to_owned()));
        }
        let mut cursor = parent;
        let mut visited = BTreeSet::new();
        while let Some(p) = cursor {
            if p == id || !visited.insert(p) {
                return Err(Error::Cycle);
            }
            let ancestor = self
                .widgets
                .get(p)
                .ok_or_else(|| Error::Missing(p.to_owned()))?;
            cursor = ancestor.parent.as_deref();
        }
        if let Some(widget) = self.widgets.get_mut(id) {
            widget.parent = parent.map(ToOwned::to_owned);
        }
        Ok(())
    }

    pub fn remove_subtree(&mut self, id: &str) -> Result<usize, Error> {
        if !self.widgets.contains_key(id) {
            return Err(Error::Missing(id.to_owned()));
        }
        let mut removed = BTreeSet::new();
        removed.insert(id.to_owned());
        loop {
            let previous = removed.len();
            for widget in self.widgets.values() {
                if widget.parent.as_ref().is_some_and(|p| removed.contains(p)) {
                    removed.insert(widget.id.clone());
                }
            }
            if previous == removed.len() {
                break;
            }
        }
        self.widgets.retain(|key, _| !removed.contains(key));
        self.events
            .retain(|event| !removed.contains(&event.widget_id));
        Ok(removed.len())
    }

    fn available(&self, id: &str) -> Result<bool, Error> {
        let mut current = Some(id);
        let mut visited = BTreeSet::new();
        while let Some(key) = current {
            if !visited.insert(key) {
                return Err(Error::Cycle);
            }
            let widget = self
                .widgets
                .get(key)
                .ok_or_else(|| Error::Missing(key.to_owned()))?;
            if !widget.visible || !widget.enabled {
                return Ok(false);
            }
            current = widget.parent.as_deref();
        }
        Ok(true)
    }

    /// A native host invokes this on actual user input, never synthetic renderer actions.
    pub fn perform(&mut self, id: &str, action: Action) -> Result<Event, Error> {
        if !self.available(id)? {
            return Err(Error::Unavailable(id.to_owned()));
        }
        let widget = self
            .widgets
            .get_mut(id)
            .ok_or_else(|| Error::Missing(id.to_owned()))?;
        if widget.read_only && !matches!(action, Action::Activate) {
            return Err(Error::ReadOnly(id.to_owned()));
        }
        match (widget.kind, &action) {
            (Kind::Button | Kind::Tile, Action::Activate) => {}
            (Kind::TextField, Action::Input(text)) => widget.text = text.clone(),
            (Kind::Toggle, Action::Toggle(state)) => widget.checked = *state,
            (Kind::Selector | Kind::Tabs | Kind::Menu, Action::Select(value)) => {
                if !widget.options.contains(value) {
                    return Err(Error::InvalidOption(value.clone()));
                }
                widget.selected = Some(value.clone());
            }
            (Kind::Dialog, Action::Show) => widget.open = true,
            (Kind::Dialog, Action::Hide) => widget.open = false,
            _ => return Err(Error::InvalidAction),
        }
        let event = Event {
            widget_id: id.to_owned(),
            action,
        };
        self.events.push(event.clone());
        Ok(event)
    }

    /// Mount the genuine FrnlRS event path using a host-supplied callback.
    pub fn mount_frnl(
        &self,
        surface: &mut Surface,
        id: &str,
        handler: impl FnMut(Control, &Value) -> Result<(), SurfaceError> + 'static,
    ) -> Result<(), SurfaceError> {
        let widget = self
            .widgets
            .get(id)
            .ok_or_else(|| SurfaceError::Unknown(id.to_owned()))?;
        let controls = match widget.kind {
            Kind::Button | Kind::Tile | Kind::Dialog => alloc::vec![Control::Click],
            Kind::TextField => alloc::vec![Control::Input],
            Kind::Toggle | Kind::Selector | Kind::Tabs | Kind::Menu => {
                alloc::vec![Control::Change]
            }
            Kind::Label => Vec::new(),
        };
        let mut descriptor = Descriptor::new(id.to_owned(), controls);
        descriptor.parent_id = widget.parent.clone();
        descriptor.visible = widget.visible;
        descriptor.enabled = widget.enabled;
        descriptor.read_only = widget.read_only;
        surface.mount(descriptor, handler)
    }
}
