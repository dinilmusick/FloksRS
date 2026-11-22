//! Renderer-independent musical models. Pitch 0..127, integer MIDI ticks.
use alloc::borrow::ToOwned;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    EmptyId,
    Duplicate(String),
    Missing(String),
    Pitch,
    Velocity,
    Duration,
    Overflow,
    Loop,
    Geometry,
    TimeRange,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub id: String,
    pub pitch: u8,
    pub start_tick: u64,
    pub duration_ticks: u64,
    pub velocity: u8,
}

impl Note {
    pub fn validate(&self) -> Result<(), Error> {
        if self.id.trim().is_empty() {
            return Err(Error::EmptyId);
        }
        if self.pitch > 127 {
            return Err(Error::Pitch);
        }
        if self.velocity == 0 || self.velocity > 127 {
            return Err(Error::Velocity);
        }
        if self.duration_ticks == 0 {
            return Err(Error::Duration);
        }
        self.start_tick
            .checked_add(self.duration_ticks)
            .ok_or(Error::Overflow)?;
        Ok(())
    }

    pub fn end_tick(&self) -> u64 {
        self.start_tick + self.duration_ticks
    }
}

#[derive(Clone, Debug, Default)]
pub struct PianoRoll {
    notes: BTreeMap<String, Note>,
    pub playhead_tick: u64,
    pub loop_range: Option<(u64, u64)>,
    pub quantization_ticks: u64,
}

impl PianoRoll {
    pub fn insert(&mut self, note: Note) -> Result<(), Error> {
        note.validate()?;
        if self.notes.contains_key(&note.id) {
            return Err(Error::Duplicate(note.id));
        }
        self.notes.insert(note.id.clone(), note);
        Ok(())
    }

    pub fn notes(&self) -> Vec<&Note> {
        self.notes.values().collect()
    }

    pub fn get(&self, id: &str) -> Option<&Note> {
        self.notes.get(id)
    }

    pub fn remove(&mut self, id: &str) -> Result<Note, Error> {
        self.notes
            .remove(id)
            .ok_or_else(|| Error::Missing(id.to_owned()))
    }

    pub fn move_note(&mut self, id: &str, pitch: u8, tick: u64) -> Result<(), Error> {
        let mut note = self
            .notes
            .get(id)
            .cloned()
            .ok_or_else(|| Error::Missing(id.to_owned()))?;
        note.pitch = pitch;
        note.start_tick = tick;
        note.validate()?;
        self.notes.insert(id.to_owned(), note);
        Ok(())
    }

    pub fn resize_note(&mut self, id: &str, ticks: u64) -> Result<(), Error> {
        let mut note = self
            .notes
            .get(id)
            .cloned()
            .ok_or_else(|| Error::Missing(id.to_owned()))?;
        note.duration_ticks = ticks;
        note.validate()?;
        self.notes.insert(id.to_owned(), note);
        Ok(())
    }

    pub fn set_loop(&mut self, range: Option<(u64, u64)>) -> Result<(), Error> {
        if range.is_some_and(|(start, end)| start >= end) {
            return Err(Error::Loop);
        }
        self.loop_range = range;
        Ok(())
    }

    pub fn snap(&self, tick: u64) -> u64 {
        if self.quantization_ticks == 0 {
            tick
        } else {
            tick - tick % self.quantization_ticks
        }
    }

    pub fn set_playhead(&mut self, tick: u64) {
        self.playhead_tick = tick;
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry {
    pub origin_x: f64,
    pub origin_y: f64,
    pub pixels_per_tick: f64,
    pub pitch_height: f64,
}

impl Geometry {
    pub fn validate(self) -> Result<(), Error> {
        if !self.origin_x.is_finite()
            || !self.origin_y.is_finite()
            || !self.pixels_per_tick.is_finite()
            || self.pixels_per_tick <= 0.0
            || !self.pitch_height.is_finite()
            || self.pitch_height <= 0.0
        {
            return Err(Error::Geometry);
        }
        Ok(())
    }

    /// 127 is the top row; 0 is the bottom row.
    pub fn note_to_xy(self, pitch: u8, tick: u64) -> Result<(f64, f64), Error> {
        self.validate()?;
        if pitch > 127 {
            return Err(Error::Pitch);
        }
        let x = self.origin_x + (tick as f64) * self.pixels_per_tick;
        let y = self.origin_y + f64::from(127 - pitch) * self.pitch_height;
        if !x.is_finite() || !y.is_finite() {
            return Err(Error::Geometry);
        }
        Ok((x, y))
    }

    pub fn xy_to_pitch_tick(self, x: f64, y: f64) -> Result<(u8, u64), Error> {
        self.validate()?;
        if !x.is_finite() || !y.is_finite() {
            return Err(Error::Geometry);
        }
        let xt = (x - self.origin_x) / self.pixels_per_tick;
        let yp = (y - self.origin_y) / self.pitch_height;
        if !xt.is_finite()
            || !yp.is_finite()
            || !(0.0..128.0).contains(&yp)
            || xt < 0.0
            || xt >= (u64::MAX as f64)
        {
            return Err(Error::TimeRange);
        }
        Ok((127 - yp as u8, xt as u64))
    }

    pub fn zoom_about_x(self, anchor_x: f64, factor: f64) -> Result<Self, Error> {
        self.validate()?;
        if !anchor_x.is_finite() || !factor.is_finite() || factor <= 0.0 {
            return Err(Error::Geometry);
        }
        let world_tick = (anchor_x - self.origin_x) / self.pixels_per_tick;
        let pixels_per_tick = self.pixels_per_tick * factor;
        let next = Self {
            origin_x: anchor_x - world_tick * pixels_per_tick,
            pixels_per_tick,
            ..self
        };
        next.validate()?;
        Ok(next)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub id: String,
    pub label: String,
    pub start_tick: u64,
    pub end_tick: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Timeline {
    sections: BTreeMap<String, Section>,
    pub playhead_tick: u64,
}

impl Timeline {
    pub fn insert(&mut self, section: Section) -> Result<(), Error> {
        if section.id.trim().is_empty() {
            return Err(Error::EmptyId);
        }
        if section.end_tick <= section.start_tick {
            return Err(Error::TimeRange);
        }
        if self.sections.contains_key(&section.id) {
            return Err(Error::Duplicate(section.id));
        }
        self.sections.insert(section.id.clone(), section);
        Ok(())
    }

    pub fn list(&self) -> Vec<&Section> {
        self.sections.values().collect()
    }

    pub fn remove(&mut self, id: &str) -> Result<Section, Error> {
        self.sections
            .remove(id)
            .ok_or_else(|| Error::Missing(id.to_owned()))
    }

    pub fn sections_at(&self, tick: u64) -> Vec<&Section> {
        self.sections
            .values()
            .filter(|section| tick >= section.start_tick && tick < section.end_tick)
            .collect()
    }
}
