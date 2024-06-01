use anyhow::{bail, Context, Error, Result};
use crossbeam_channel::{unbounded, Receiver, Sender, TrySendError};
use midir::{MidiOutput, MidiOutputPort};
use std::time::Instant;
use time::ext::NumericalDuration;

use crate::midisync::MidiSync;

pub enum MultiSyncCommand {
    Start,
    Stop,
    AddListener(Sender<MultiSyncEvent>),
}

#[derive(Clone, Copy)]
pub enum MultiSyncEvent {
    Started(Instant),
    Stopped,
}

pub struct MultiSyncCtrl {
    listeners: Vec<Sender<MultiSyncEvent>>,
    cmd: Receiver<MultiSyncCommand>,
}

pub struct MultiSync {
    ctrl: MultiSyncCtrl,
    port_enum: MidiOutput, // Client used to enumerate available ports
    clients: Vec<MultiSyncMidiClient>,
}

pub struct PortInfo {
    port: MidiOutputPort,
    name: String,
}

pub struct MultiSyncMidiClient {
    info: PortInfo,
    sync: Option<MidiSync>,
}

impl MultiSyncCtrl {
    pub fn new() -> (MultiSyncCtrl, Sender<MultiSyncCommand>) {
        let (s_cmd, r_cmd) = unbounded();
        (
            MultiSyncCtrl {
                listeners: Vec::new(),
                cmd: r_cmd,
            },
            s_cmd,
        )
    }

    pub fn publish(&mut self, msg: MultiSyncEvent) {
        self.listeners
            .retain(|tx| !matches!(tx.try_send(msg), Err(TrySendError::Disconnected(_))))
    }

    pub fn get_cmd(&mut self) -> Option<MultiSyncCommand> {
        match self.cmd.try_recv().ok() {
            Some(MultiSyncCommand::AddListener(listener)) => {
                self.listeners.push(listener);
                None
            }
            x => x,
        }
    }
}

impl MultiSync {
    pub fn new() -> Result<(MultiSync, Sender<MultiSyncCommand>)> {
        let (ctrl, cmd) = MultiSyncCtrl::new();
        let port_enum =
            MidiOutput::new("MultiSync Controller").context("Failed to create MidiOutput")?;
        Ok((
            MultiSync {
                ctrl,
                port_enum,
                clients: Vec::new(),
            },
            cmd,
        ))
    }

    pub fn run(&mut self) -> Result<()> {
        self.clients
            .iter_mut()
            .filter_map(|c| c.sync.as_mut())
            .for_each(|s| s.run());

        self.update_ports();

        Ok(())
    }

    fn update_ports(&mut self) -> Result<()> {
        let ports = self.port_enum.ports();
        let existing_ports = self.clients.iter().map(|c| &c.info.port);
        let new_ports: Vec<MidiOutputPort> = ports
            .clone()
            .into_iter()
            .filter(|p| existing_ports.clone().find(|ep| p == *ep).is_none())
            .collect();
        Ok(())
    }
}
