//! Token-based appearance and layout descriptors. No native renderer is simulated.
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Appearance {
    Light,
    Dark,
    System,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolvedAppearance {
    Light,
    Dark,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    pub appearance: Appearance,
    pub tokens: BTreeMap<String, String>,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            appearance: Appearance::System,
            tokens: BTreeMap::new(),
        }
    }
}

impl Theme {
    pub fn resolve(&self, system_dark: bool) -> ResolvedAppearance {
        match self.appearance {
            Appearance::Light => ResolvedAppearance::Light,
            Appearance::Dark => ResolvedAppearance::Dark,
            Appearance::System if system_dark => ResolvedAppearance::Dark,
            Appearance::System => ResolvedAppearance::Light,
        }
    }

    pub fn token(&self, name: &str) -> Option<&str> {
        self.tokens.get(name).map(String::as_str)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Row,
    Column,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub direction: Direction,
    pub padding: u32,
    pub gap: u32,
    pub limit: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutError {
    Overflow,
    ExceedsLimit,
}

impl Layout {
    pub fn place(self, children: &[(u32, u32)]) -> Result<Vec<Rect>, LayoutError> {
        let mut cursor = self.padding;
        let mut out = Vec::new();
        for &(width, height) in children {
            let (x, y) = match self.direction {
                Direction::Row => (cursor, self.padding),
                Direction::Column => (self.padding, cursor),
            };
            let bound = cursor
                .checked_add(match self.direction {
                    Direction::Row => width,
                    Direction::Column => height,
                })
                .ok_or(LayoutError::Overflow)?;
            if self.limit.is_some_and(|limit| bound > limit) {
                return Err(LayoutError::ExceedsLimit);
            }
            out.push(Rect {
                x,
                y,
                width,
                height,
            });
            cursor = bound.checked_add(self.gap).ok_or(LayoutError::Overflow)?;
        }
        Ok(out)
    }
}

/// Hosts implement real drawing and resource lifetime.
pub trait NativeRenderer {
    type Error;

    fn draw(
        &mut self,
        widgets: &crate::controls::Registry,
        theme: &Theme,
        rectangles: &[Rect],
    ) -> Result<(), Self::Error>;
}
