use floksrs::controls::{Action, Kind, Registry, Widget};
use floksrs::framebuffer::Framebuffer;
use floksrs::host_io::{send_midi, send_osc, MidiPacket, MidiPort, OscArg, OscMessage, OscPort};
use floksrs::semantic_bridge::Bridge;
use floksrs::theme::{NativeRenderer, Rect, Theme};
use floksrs::music::{Geometry, Note, PianoRoll};
use frnlrs::{Control, Surface, SurfaceError, Value};
use std::cell::Cell;
use std::rc::Rc;

#[derive(Default)]
struct HostPort(Vec<u8>);
impl MidiPort for HostPort {
    type Error = ();
    fn send(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        self.0.extend_from_slice(bytes);
        Ok(())
    }
}
impl OscPort for HostPort {
    type Error = ();
    fn send(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        self.0.extend_from_slice(bytes);
        Ok(())
    }
}

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
    // Independent host delivery: real FrnlRS lifecycle/state and event history.
    let mut bridge = Bridge::new();
    bridge.mount(Widget::new("native-action", Kind::Button, None)).unwrap();
    bridge.dispatch("native-action", Action::Activate).unwrap();
    assert_eq!(bridge.runtime.history().len(), 1);
    assert_eq!(bridge.dispose_subtree("native-action").unwrap(), 1);

    // Real RGB framebuffer mutation, not a fake presentation acknowledgement.
    let mut framebuffer_pixels = vec![0u32; 64 * 20];
    let mut framebuffer = Framebuffer::new(64, 20, &mut framebuffer_pixels).unwrap();
    framebuffer.draw(
        &registry,
        &Theme::default(),
        &[Rect { x: 0, y: 0, width: 64, height: 20 }],
    ).unwrap();
    assert_ne!(framebuffer.pixels()[0], 0);

    // Actual host-provided MIDI/OSC byte delivery and OSC decoding.
    let mut midi_host = HostPort::default();
    send_midi(&mut midi_host, &MidiPacket::note_on(0, 60, 100).unwrap()).unwrap();
    assert_eq!(midi_host.0, vec![0x90, 60, 100]);
    let message = OscMessage {
        address: "/piano/note".into(),
        args: vec![OscArg::Int(60)],
    };
    let mut osc_host = HostPort::default();
    send_osc(&mut osc_host, &message).unwrap();
    assert_eq!(OscMessage::decode(&osc_host.0).unwrap(), message);
    println!("FloksRS and FrnlRS native consumer PASS");
}
