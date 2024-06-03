use anyhow::{bail, Context, Error, Result};
use crossbeam_channel::{unbounded, Receiver, Sender, TrySendError};
use midir::{MidiInputPort, MidiOutput, MidiOutputPort};
use std::time::Duration;
use time::ext::NumericalStdDuration;

use crate::midisync::MidiSync;

pub enum MultiSyncCommand {
    Start,
    Stop,
    AddListener(Sender<MultiSyncEvent>),
    AddSyncForPort(MidiOutputPort)
}

#[derive(Clone, Copy)]
pub enum MultiSyncEvent {
    Started(Duration),
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
        self.process_cmds();

        Ok(())
    }

    fn update_ports(&mut self) -> Result<()> {
        let ports = self.port_enum.ports();
        let existing_ports = self.clients.iter().map(|c| &c.info.port);

        let new_ports = ports
            .clone()
            .into_iter()
            .filter(|p| existing_ports.clone().find(|ep| p == *ep).is_none());

        /* TODO: Matchup new ports with ports that have lost connection, this will require some fuzzy matching 
         *       since the names (at least for ALSA) will likely change */

        let new_port_info: Vec<PortInfo> = new_ports.flat_map(|port| -> Result<PortInfo> {
            let name = self.port_enum.port_name(&port)?;
            Ok(PortInfo { port, name })
        }).collect();

        let has_new_ports = !new_port_info.is_empty();

        self.clients.extend( new_port_info.into_iter().map(|p| MultiSyncMidiClient { info: p, sync: None }));

        Ok(())
    }

    fn process_cmds(&mut self) -> Result <()> {
        while let Some(cmd) = self.ctrl.get_cmd() {
            let result = match cmd {
                MultiSyncCommand::AddSyncForPort(port) => self.add_sync_for_port(port),
                _ => Ok(())
            };
        };
        Ok(())
    }

    fn add_sync_for_port(&mut self, port: MidiOutputPort) -> Result<()> {
        let client = self.clients.iter_mut().find(|p| p.info.port == port).context("Port not found")?;
        if client.sync.is_some() {
            bail!("Port already connected to client");
        }
        let midi_out =midir::MidiOutput::new("Midimaxe Sync Client")?.connect(&client.info.port, "Midimaxe Sync Client Port");
        if midi_out.is_err() {
            bail!("Failed to connect to MIDI output");
        }
        // TODO: Change BPM to timeline here
        client.sync = Some(MidiSync::new(midi_out.unwrap() , 130.0, None));
        Ok(())
    }
}
