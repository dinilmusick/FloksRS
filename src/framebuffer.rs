//! Real pixel renderer for native/embedded RGB888 framebuffers.
//! The host owns the memory and display I/O; rendering mutates observable pixels.
use crate::controls::{Kind, Registry};
use crate::theme::{NativeRenderer, Rect, Theme};
use alloc::vec::Vec;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    BufferSize,
    Overflow,
    LayoutMismatch,
    InvalidToken,
}

pub struct Framebuffer<'a> {
    pub width: u32,
    pub height: u32,
    pixels: &'a mut [u32],
}

fn color(theme: &Theme, key: &str, fallback: u32) -> Result<u32, Error> {
    let Some(value) = theme.token(key) else {
        return Ok(fallback);
    };
    if value.len() != 7 || !value.starts_with('#') {
        return Err(Error::InvalidToken);
    }
    u32::from_str_radix(&value[1..], 16).map_err(|_| Error::InvalidToken)
}

impl<'a> Framebuffer<'a> {
    pub fn new(width: u32, height: u32, pixels: &'a mut [u32]) -> Result<Self, Error> {
        let size = (width as usize)
            .checked_mul(height as usize)
            .ok_or(Error::Overflow)?;
        if size != pixels.len() {
            return Err(Error::BufferSize);
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub fn pixels(&self) -> &[u32] {
        self.pixels
    }

    fn pixel(&mut self, x: u32, y: u32, rgb: u32) {
        if x < self.width && y < self.height {
            let offset = y as usize * self.width as usize + x as usize;
            self.pixels[offset] = rgb;
        }
    }

    fn fill_rect(&mut self, rect: Rect, rgb: u32) {
        let x2 = rect.x.saturating_add(rect.width).min(self.width);
        let y2 = rect.y.saturating_add(rect.height).min(self.height);
        for y in rect.y.min(self.height)..y2 {
            for x in rect.x.min(self.width)..x2 {
                self.pixel(x, y, rgb);
            }
        }
    }

    fn border(&mut self, r: Rect, rgb: u32) {
        if r.width == 0 || r.height == 0 {
            return;
        }
        self.fill_rect(
            Rect {
                x: r.x,
                y: r.y,
                width: r.width,
                height: 1,
            },
            rgb,
        );
        self.fill_rect(
            Rect {
                x: r.x,
                y: r.y.saturating_add(r.height.saturating_sub(1)),
                width: r.width,
                height: 1,
            },
            rgb,
        );
        self.fill_rect(
            Rect {
                x: r.x,
                y: r.y,
                width: 1,
                height: r.height,
            },
            rgb,
        );
        self.fill_rect(
            Rect {
                x: r.x.saturating_add(r.width.saturating_sub(1)),
                y: r.y,
                width: 1,
                height: r.height,
            },
            rgb,
        );
    }

    fn glyph(&mut self, x: u32, y: u32, ch: char, rgb: u32) {
        // Five column, seven row bitmap. Unsupported glyphs are visibly blank.
        let columns: [u8; 5] = match ch.to_ascii_uppercase() {
            'A' => [0x7e, 0x11, 0x11, 0x11, 0x7e],
            'B' => [0x7f, 0x49, 0x49, 0x49, 0x36],
            'C' => [0x3e, 0x41, 0x41, 0x41, 0x22],
            'D' => [0x7f, 0x41, 0x41, 0x22, 0x1c],
            'E' => [0x7f, 0x49, 0x49, 0x49, 0x41],
            'F' => [0x7f, 0x09, 0x09, 0x09, 0x01],
            'G' => [0x3e, 0x41, 0x49, 0x49, 0x7a],
            'H' => [0x7f, 0x08, 0x08, 0x08, 0x7f],
            'I' => [0x41, 0x41, 0x7f, 0x41, 0x41],
            'J' => [0x20, 0x40, 0x41, 0x3f, 0x01],
            'K' => [0x7f, 0x08, 0x14, 0x22, 0x41],
            'L' => [0x7f, 0x40, 0x40, 0x40, 0x40],
            'M' => [0x7f, 0x02, 0x0c, 0x02, 0x7f],
            'N' => [0x7f, 0x04, 0x08, 0x10, 0x7f],
            'O' => [0x3e, 0x41, 0x41, 0x41, 0x3e],
            'P' => [0x7f, 0x09, 0x09, 0x09, 0x06],
            'Q' => [0x3e, 0x41, 0x51, 0x21, 0x5e],
            'R' => [0x7f, 0x09, 0x19, 0x29, 0x46],
            'S' => [0x46, 0x49, 0x49, 0x49, 0x31],
            'T' => [0x01, 0x01, 0x7f, 0x01, 0x01],
            'U' => [0x3f, 0x40, 0x40, 0x40, 0x3f],
            'V' => [0x1f, 0x20, 0x40, 0x20, 0x1f],
            'W' => [0x7f, 0x20, 0x18, 0x20, 0x7f],
            'X' => [0x63, 0x14, 0x08, 0x14, 0x63],
            'Y' => [0x03, 0x04, 0x78, 0x04, 0x03],
            'Z' => [0x61, 0x51, 0x49, 0x45, 0x43],
            '0' => [0x3e, 0x45, 0x49, 0x51, 0x3e],
            '1' => [0x00, 0x42, 0x7f, 0x40, 0x00],
            '2' => [0x42, 0x61, 0x51, 0x49, 0x46],
            '3' => [0x21, 0x41, 0x45, 0x4b, 0x31],
            '4' => [0x18, 0x14, 0x12, 0x7f, 0x10],
            '5' => [0x27, 0x45, 0x45, 0x45, 0x39],
            '6' => [0x3c, 0x4a, 0x49, 0x49, 0x30],
            '7' => [0x01, 0x71, 0x09, 0x05, 0x03],
            '8' => [0x36, 0x49, 0x49, 0x49, 0x36],
            '9' => [0x06, 0x49, 0x49, 0x29, 0x1e],
            '-' => [0x08, 0x08, 0x08, 0x08, 0x08],
            '.' => [0, 0x60, 0x60, 0, 0],
            ':' => [0, 0x36, 0x36, 0, 0],
            '/' => [0x60, 0x10, 0x08, 0x04, 0x03],
            '!' => [0, 0, 0x5f, 0, 0],
            _ => [0; 5],
        };
        for (dx, bits) in columns.into_iter().enumerate() {
            for dy in 0..7u32 {
                if bits & (1 << dy) != 0 {
                    self.pixel(x.saturating_add(dx as u32), y.saturating_add(dy), rgb);
                }
            }
        }
    }

    fn label(&mut self, rect: Rect, text: &str, color: u32) {
        let mut x = rect.x.saturating_add(3);
        let max = rect.x.saturating_add(rect.width).min(self.width);
        let y = rect.y.saturating_add(3);
        for c in text.chars() {
            if x.saturating_add(5) >= max {
                break;
            }
            self.glyph(x, y, c, color);
            x = x.saturating_add(6);
        }
    }
}

impl NativeRenderer for Framebuffer<'_> {
    type Error = Error;
    fn draw(
        &mut self,
        widgets: &Registry,
        theme: &Theme,
        rectangles: &[Rect],
    ) -> Result<(), Self::Error> {
        let snapshots = widgets.snapshot();
        if snapshots.len() != rectangles.len() {
            return Err(Error::LayoutMismatch);
        }
        let bg = color(theme, "ui.background", 0x101010)?;
        let fg = color(theme, "ui.foreground", 0xffffff)?;
        let border = color(theme, "ui.border", 0x808080)?;
        let normal = color(theme, "ui.control", 0x202b40)?;
        let checked = color(theme, "ui.checked", 0x176a45)?;
        let disabled = color(theme, "ui.disabled", 0x393939)?;
        let palette: Vec<(bool, bool, u32)> = snapshots
            .iter()
            .map(|w| {
                (
                    w.visible,
                    w.enabled,
                    if !w.enabled {
                        disabled
                    } else if w.checked || w.open {
                        checked
                    } else {
                        normal
                    },
                )
            })
            .collect();
        self.pixels.fill(bg);
        for ((w, rect), (visible, _, fill)) in snapshots.iter().zip(rectangles).zip(palette) {
            if !visible {
                continue;
            }
            self.fill_rect(*rect, fill);
            self.border(*rect, border);
            if w.kind == Kind::Toggle && w.checked {
                self.fill_rect(
                    Rect {
                        x: rect.x.saturating_add(2),
                        y: rect.y.saturating_add(2),
                        width: 3,
                        height: 3,
                    },
                    fg,
                );
            }
            self.label(*rect, &w.label, fg);
        }
        Ok(())
    }
}
