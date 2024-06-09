use std::time::Duration;

pub const MIDI_START: [u8; 1] = [250];
pub const MIDI_STOP: [u8; 1] = [252];
pub const MIDI_CLOCK: [u8; 1] = [248];

#[derive(Debug, Clone)]
pub enum MidiRealtimeMessage {
    MidiStart(Duration),
    MidiStop(Duration),
    MidiClock(Duration),
}

impl MidiRealtimeMessage {
    pub fn from_midi(micros: u64, data: &[u8]) -> Option<MidiRealtimeMessage> {
        let t = Duration::from_micros(micros);
        if data == MIDI_START.as_ref() {
            Some(MidiRealtimeMessage::MidiStart(t))
        } else if data == MIDI_STOP.as_ref() {
            Some(MidiRealtimeMessage::MidiStop(t))
        } else if data == MIDI_CLOCK.as_ref() {
            Some(MidiRealtimeMessage::MidiClock(t))
        } else {
            None
        }
    }

    pub fn to_midi(&self) -> &[u8] {
        match self {
            MidiRealtimeMessage::MidiClock(_) => MIDI_CLOCK.as_ref(),
            MidiRealtimeMessage::MidiStart(_) => MIDI_START.as_ref(),
            MidiRealtimeMessage::MidiStop(_) => MIDI_STOP.as_ref(),
        }
    }
}
