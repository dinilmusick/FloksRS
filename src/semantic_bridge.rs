//! Floks control semantics composed with FrnlRS lifecycle, state and event runtime.
//! Events originate in the host; the bridge never schedules or synthesizes input.
use crate::controls::{Action, Error as UiError, Event, Kind, Registry, Widget};
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use frnlrs::semantic::{
    ComponentDefinition, Context, Data, DataKind, Error as FrnlError, EventDefinition,
    LifecyclePhase, Runtime, StateDefinition,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Widget(UiError),
    Runtime(FrnlError),
    NotActive(String),
}
impl From<UiError> for Error {
    fn from(e: UiError) -> Self {
        Self::Widget(e)
    }
}
impl From<FrnlError> for Error {
    fn from(e: FrnlError) -> Self {
        Self::Runtime(e)
    }
}

/// A native host owns this bridge and calls dispatch only for observed user actions.
pub struct Bridge {
    pub controls: Registry,
    pub runtime: Runtime,
}
impl Default for Bridge {
    fn default() -> Self {
        Self {
            controls: Registry::default(),
            runtime: Runtime::new(),
        }
    }
}
impl Bridge {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mount(&mut self, widget: Widget) -> Result<(), Error> {
        let mut staged = self.controls.clone();
        staged.insert(widget.clone())?;
        let id = widget.id.clone();
        let definition_name = format!("floks::{id}");
        let mut events = BTreeMap::new();
        let specs: Vec<(&str, DataKind)> = match widget.kind {
            Kind::Button | Kind::Tile => alloc::vec![("activate", DataKind::Null)],
            Kind::TextField => alloc::vec![("input", DataKind::Text)],
            Kind::Toggle => alloc::vec![("toggle", DataKind::Bool)],
            Kind::Menu | Kind::Selector | Kind::Tabs => alloc::vec![("select", DataKind::Text)],
            Kind::Dialog => alloc::vec![("show", DataKind::Bool), ("hide", DataKind::Bool)],
            Kind::Label => Vec::new(),
        };
        for &(name, payload) in &specs {
            events.insert(
                name.to_string(),
                EventDefinition {
                    function_id: format!("floks_{name}"),
                    payload,
                },
            );
        }
        let mut states = BTreeMap::new();
        states.insert(
            "idle".into(),
            StateDefinition {
                events: events.clone(),
                ..StateDefinition::default()
            },
        );
        states.insert(
            "active".into(),
            StateDefinition {
                events,
                ..StateDefinition::default()
            },
        );
        self.runtime.register_definition(ComponentDefinition {
            name: definition_name.clone(),
            initial_state: "idle".into(),
            states,
            context: Context::default(),
        })?;
        self.runtime
            .mount(&id, &definition_name, widget.parent.as_deref())?;
        self.runtime
            .transition_lifecycle(&id, LifecyclePhase::Initializing)?;
        self.runtime
            .transition_lifecycle(&id, LifecyclePhase::Mounted)?;
        self.runtime
            .transition_lifecycle(&id, LifecyclePhase::Active)?;
        for &(event, _) in &specs {
            let name = event.to_string();
            self.runtime.register_handler(
                &id,
                &format!("floks_{event}"),
                move |payload, context| {
                    context.set_data("value", payload.clone());
                    context.set_state(if name == "hide" { "idle" } else { "active" });
                    Ok(payload.clone())
                },
            )?;
        }
        self.controls = staged;
        Ok(())
    }

    pub fn dispatch(&mut self, id: &str, action: Action) -> Result<Event, Error> {
        if self.runtime.entity(id)?.lifecycle != LifecyclePhase::Active {
            return Err(Error::NotActive(id.to_string()));
        }
        let mut staged = self.controls.clone();
        let event = staged.perform(id, action.clone())?;
        let (name, data) = match action {
            Action::Activate => ("activate", Data::Null),
            Action::Input(text) => ("input", Data::Text(text)),
            Action::Select(text) => ("select", Data::Text(text)),
            Action::Toggle(value) => ("toggle", Data::Bool(value)),
            Action::Show => ("show", Data::Bool(true)),
            Action::Hide => ("hide", Data::Bool(false)),
        };
        self.runtime.emit(id, name, data)?;
        self.controls = staged;
        Ok(event)
    }

    pub fn suspend(&mut self, id: &str) -> Result<(), Error> {
        self.runtime
            .transition_lifecycle(id, LifecyclePhase::Suspended)?;
        Ok(())
    }
    pub fn resume(&mut self, id: &str) -> Result<(), Error> {
        self.runtime
            .transition_lifecycle(id, LifecyclePhase::Resuming)?;
        self.runtime
            .transition_lifecycle(id, LifecyclePhase::Active)?;
        Ok(())
    }
    pub fn advance_to(&mut self, timestamp_ms: u64) -> Result<(), Error> {
        self.runtime.advance_to(timestamp_ms)?;
        Ok(())
    }

    /// Dispose every descendant first, including Frnl handlers and queued events.
    pub fn dispose_subtree(&mut self, root: &str) -> Result<usize, Error> {
        if self.controls.get(root).is_none() {
            return Err(UiError::Missing(root.to_string()).into());
        }
        let mut removed = Vec::new();
        for w in self.controls.snapshot() {
            let mut current = Some(w.id.as_str());
            let mut visited = BTreeSet::new();
            while let Some(id) = current {
                if !visited.insert(id) {
                    break;
                }
                if id == root {
                    removed.push(w.id.clone());
                    break;
                }
                current = self.controls.get(id).and_then(|n| n.parent.as_deref());
            }
        }
        let mut ordered = Vec::new();
        for id in removed {
            ordered.push((self.runtime.path(&id)?.len(), id));
        }
        ordered.sort_by_key(|a| core::cmp::Reverse(a.0));
        for (_, id) in &ordered {
            self.runtime
                .transition_lifecycle(id, LifecyclePhase::Disposing)?;
            self.runtime
                .transition_lifecycle(id, LifecyclePhase::Disposed)?;
            self.runtime.unmount(id)?;
        }
        self.controls.remove_subtree(root)?;
        Ok(ordered.len())
    }
}
