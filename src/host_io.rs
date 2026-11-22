//! Native MIDI and OSC packet contracts. The caller owns devices and networking.
//! No resident runtime, network socket, scheduler, or simulated device is created.
use alloc::string::{String, ToString};
use alloc::vec::Vec;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    InvalidMidiStatus,
    InvalidMidiData,
    InvalidChannel,
    InvalidOscAddress,
    InvalidOscArgument,
    Truncated,
    Oversized,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MidiPacket {
    bytes: [u8; 3],
    length: usize,
}
impl MidiPacket {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() < 2 || bytes.len() > 3 {
            return Err(Error::InvalidMidiData);
        }
        let status = bytes[0];
        if !(0x80..=0xef).contains(&status) {
            return Err(Error::InvalidMidiStatus);
        }
        let expected = if (status & 0xf0) == 0xc0 || (status & 0xf0) == 0xd0 {
            2
        } else {
            3
        };
        if bytes.len() != expected || bytes[1..].iter().any(|v| *v > 127) {
            return Err(Error::InvalidMidiData);
        }
        let mut array = [0u8; 3];
        array[..expected].copy_from_slice(bytes);
        Ok(Self {
            bytes: array,
            length: expected,
        })
    }
    pub fn note_on(channel: u8, pitch: u8, velocity: u8) -> Result<Self, Error> {
        if channel > 15 {
            return Err(Error::InvalidChannel);
        }
        Self::from_bytes(&[0x90 | channel, pitch, velocity])
    }
    pub fn note_off(channel: u8, pitch: u8, velocity: u8) -> Result<Self, Error> {
        if channel > 15 {
            return Err(Error::InvalidChannel);
        }
        Self::from_bytes(&[0x80 | channel, pitch, velocity])
    }
    pub fn control_change(channel: u8, control: u8, value: u8) -> Result<Self, Error> {
        if channel > 15 {
            return Err(Error::InvalidChannel);
        }
        Self::from_bytes(&[0xb0 | channel, control, value])
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
}

pub trait MidiPort {
    type Error;
    /// A real native/embedded driver implements this method.
    fn send(&mut self, bytes: &[u8]) -> Result<(), Self::Error>;
}
pub fn send_midi<P: MidiPort>(port: &mut P, packet: &MidiPacket) -> Result<(), P::Error> {
    port.send(packet.as_bytes())
}

#[derive(Clone, Debug, PartialEq)]
pub enum OscArg {
    Int(i32),
    Float(f32),
    Text(String),
    Bool(bool),
}
#[derive(Clone, Debug, PartialEq)]
pub struct OscMessage {
    pub address: String,
    pub args: Vec<OscArg>,
}
fn validate_address(value: &str) -> Result<(), Error> {
    if !value.starts_with('/')
        || value.len() < 2
        || value.bytes().any(|b| b == 0 || b.is_ascii_whitespace())
    {
        return Err(Error::InvalidOscAddress);
    }
    Ok(())
}
fn push_osc_string(bytes: &mut Vec<u8>, value: &str) -> Result<(), Error> {
    if value.as_bytes().contains(&0) {
        return Err(Error::InvalidOscArgument);
    }
    bytes.extend_from_slice(value.as_bytes());
    bytes.push(0);
    while bytes.len() % 4 != 0 {
        bytes.push(0);
    }
    if bytes.len() > 65536 {
        return Err(Error::Oversized);
    }
    Ok(())
}
impl OscMessage {
    /// OSC 1.0 address + type tags + 4-byte-aligned big-endian arguments.
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        validate_address(&self.address)?;
        let mut bytes = Vec::new();
        push_osc_string(&mut bytes, &self.address)?;
        let mut tags = String::from(",");
        for arg in &self.args {
            tags.push(match arg {
                OscArg::Int(_) => 'i',
                OscArg::Float(x) if x.is_finite() => 'f',
                OscArg::Float(_) => return Err(Error::InvalidOscArgument),
                OscArg::Text(_) => 's',
                OscArg::Bool(true) => 'T',
                OscArg::Bool(false) => 'F',
            });
        }
        push_osc_string(&mut bytes, &tags)?;
        for arg in &self.args {
            match arg {
                OscArg::Int(x) => bytes.extend_from_slice(&x.to_be_bytes()),
                OscArg::Float(x) => bytes.extend_from_slice(&x.to_bits().to_be_bytes()),
                OscArg::Text(text) => push_osc_string(&mut bytes, text)?,
                OscArg::Bool(_) => {}
            }
            if bytes.len() > 65536 {
                return Err(Error::Oversized);
            }
        }
        Ok(bytes)
    }
    pub fn decode(packet: &[u8]) -> Result<Self, Error> {
        if packet.len() > 65536 {
            return Err(Error::Oversized);
        }
        if packet.len() % 4 != 0 {
            return Err(Error::Truncated);
        }
        let mut at = 0;
        let address = read_string(packet, &mut at)?;
        validate_address(&address)?;
        let tags = read_string(packet, &mut at)?;
        if !tags.starts_with(',') {
            return Err(Error::InvalidOscArgument);
        }
        let mut args = Vec::new();
        for tag in tags.chars().skip(1) {
            match tag {
                'i' | 'f' => {
                    let end = at.checked_add(4).ok_or(Error::Truncated)?;
                    let part = packet.get(at..end).ok_or(Error::Truncated)?;
                    let word = u32::from_be_bytes(part.try_into().map_err(|_| Error::Truncated)?);
                    if tag == 'i' {
                        args.push(OscArg::Int(word as i32));
                    } else {
                        let value = f32::from_bits(word);
                        if !value.is_finite() {
                            return Err(Error::InvalidOscArgument);
                        }
                        args.push(OscArg::Float(value));
                    }
                    at = end;
                }
                's' => args.push(OscArg::Text(read_string(packet, &mut at)?)),
                'T' => args.push(OscArg::Bool(true)),
                'F' => args.push(OscArg::Bool(false)),
                _ => return Err(Error::InvalidOscArgument),
            }
        }
        if at != packet.len() {
            return Err(Error::InvalidOscArgument);
        }
        Ok(Self { address, args })
    }
}
fn read_string(packet: &[u8], cursor: &mut usize) -> Result<String, Error> {
    let start = *cursor;
    let remainder = packet.get(start..).ok_or(Error::Truncated)?;
    let stop = remainder
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(Error::Truncated)?;
    let text = core::str::from_utf8(&remainder[..stop]).map_err(|_| Error::InvalidOscArgument)?;
    let end = start.checked_add(stop + 1).ok_or(Error::Truncated)?;
    let aligned = end.checked_add(3).ok_or(Error::Truncated)? & !3;
    let pad = packet.get(end..aligned).ok_or(Error::Truncated)?;
    if pad.iter().any(|byte| *byte != 0) {
        return Err(Error::InvalidOscArgument);
    }
    *cursor = aligned;
    Ok(text.to_string())
}
pub trait OscPort {
    type Error;
    /// Native socket/driver decides destination, transport, and delivery.
    fn send(&mut self, packet: &[u8]) -> Result<(), Self::Error>;
}
pub fn send_osc<P: OscPort>(port: &mut P, message: &OscMessage) -> Result<(), SendError<P::Error>> {
    let encoded = message.encode().map_err(SendError::Packet)?;
    port.send(&encoded).map_err(SendError::Host)
}
#[derive(Debug)]
pub enum SendError<E> {
    Packet(Error),
    Host(E),
}
