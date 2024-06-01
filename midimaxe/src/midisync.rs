use anyhow::{bail, Context, Error, Result};
use midir::MidiOutputConnection;
use std::time::Instant;
use time::ext::NumericalDuration;

#[derive(Debug)]
pub enum MidiSyncState {
    Stopped,
    Starting,
    Running,
    Error(Error),
}

pub struct MidiSync {
    start_time: Option<Instant>,
    next_clk: Option<Instant>,
    bpm: f64,
    tpqn: f64,
    state: MidiSyncState,
    port: MidiOutputConnection,
}

const MIDI_START: [u8; 1] = [250];
const MIDI_STOP: [u8; 1] = [252];
const MIDI_CLOCK: [u8; 1] = [248];
const DEFAULT_TPQN: f64 = 24.0;

impl MidiSync {
    pub fn new(port: MidiOutputConnection, bpm: f64, tpqn: Option<f64>) -> MidiSync {
        MidiSync {
            start_time: None,
            next_clk: None,
            bpm,
            tpqn: tpqn.unwrap_or(DEFAULT_TPQN),
            state: MidiSyncState::Stopped,
            port,
        }
    }

    pub fn start(&mut self, start_time: Option<Instant>) {
        match self.state {
            MidiSyncState::Stopped => {
                self.start_time = Some(start_time.unwrap_or_else(|| Instant::now()));
                self.next_clk = self.start_time.clone();
                self.state = MidiSyncState::Starting;
            }
            _ => (),
        }
    }

    pub fn run(&mut self) {
        let result: Result<()> = match &self.state {
            MidiSyncState::Starting => self.run_starting(),
            MidiSyncState::Running => self.run_running(),
            _ => Ok(()),
        };
        match result {
            Err(e) => {
                self.state = MidiSyncState::Error(e);
            }
            _ => (),
        }
    }

    pub fn stop(&mut self) {
        match self.state {
            MidiSyncState::Running | MidiSyncState::Starting | MidiSyncState::Stopped => {
                // Send stop in all valid states since we can never be sure of the device state
                // and should provide users with an easy way to stop
                let result = self
                    .port
                    .send(&MIDI_STOP)
                    .context("Failed to send MIDI_STOP message");
                self.state = match result {
                    Ok(_) => MidiSyncState::Stopped,
                    Err(e) => MidiSyncState::Error(e),
                };
                self.start_time = None;
                self.next_clk = None;
            }
            _ => (),
        }
    }

    pub fn update(&mut self, bpm: f64, tpqn: Option<f64>) -> Result<()> {
        match self.state {
            MidiSyncState::Stopped => {
                self.bpm = bpm;
                self.tpqn = tpqn.unwrap_or(DEFAULT_TPQN);
                Ok(())
            }
            _ => bail!(
                "Update only valid in Stopped state, was in: {:?}",
                self.state
            ),
        }
    }

    fn run_starting(&mut self) -> Result<()> {
        let start_time = self
            .start_time
            .context("BUG: start_time == None unexpected in Starting state")?;
        if start_time <= Instant::now() {
            self.port
                .send(&MIDI_START)
                .context("Failed to send MIDI_START message")?;
            self.state = MidiSyncState::Running;
        }
        Ok(())
    }

    fn run_running(&mut self) -> Result<()> {
        let next_clk = self
            .next_clk
            .context("BUG: next_clk == None unexpected in Running state")?;
        if next_clk <= Instant::now() {
            self.port
                .send(&MIDI_CLOCK)
                .context("Failed to send MIDI_CLOCK message")?;
            self.next_clk = Some(next_clk + (1.0 / (self.bpm * self.tpqn)).minutes());
        }
        Ok(())
    }
}
