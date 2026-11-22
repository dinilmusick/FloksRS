use floksrs::controls::{Kind, Registry, Widget};
use floksrs::music::{Geometry, Note, PianoRoll};
use frnlrs::{Control, Surface, SurfaceError, Value};
use std::cell::Cell;
use std::rc::Rc;

fn main() {
    let mut registry = Registry::default();
    registry
        .insert(Widget::new("play", Kind::Button, None))
        .unwrap();
    let calls = Rc::new(Cell::new(0u32));
    let captured = Rc::clone(&calls);
    let mut surface = Surface::default();
    registry
        .mount_frnl(&mut surface, "play", move |control, value| {
            if control == Control::Click && *value == Value::None {
                captured.set(captured.get() + 1);
                Ok(())
            } else {
                Err(SurfaceError::InvalidPayload)
            }
        })
        .unwrap();
    surface.invoke("play", Control::Click, Value::None).unwrap();
    assert_eq!(calls.get(), 1);
    let mut roll = PianoRoll::default();
    roll.insert(Note {
        id: "C4".into(),
        pitch: 60,
        start_tick: 0,
        duration_ticks: 96,
        velocity: 100,
    })
    .unwrap();
    let geometry = Geometry {
        origin_x: 0.0,
        origin_y: 0.0,
        pixels_per_tick: 1.0,
        pitch_height: 1.0,
    };
    let (x, y) = geometry
        .note_to_xy(roll.notes()[0].pitch, roll.notes()[0].start_tick)
        .unwrap();
    assert_eq!(geometry.xy_to_pitch_tick(x, y).unwrap(), (60, 0));
    println!("FloksRS and FrnlRS native consumer PASS");
}
