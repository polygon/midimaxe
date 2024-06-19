use anyhow::{bail, Context, Result};
use crossbeam_channel::{unbounded, Receiver, Sender, TrySendError};
use midir::{MidiOutput, MidiOutputPort};
use std::time::Duration;
use time::ext::NumericalStdDuration;

use crate::midisync::{MidiSync, MidiSyncState};
use tracing::{error, info, warn};
use utils::programclock::{now, ProgramTime};

#[derive(Clone, Debug)]
pub struct PortDisplay {
    pub info: PortInfo,
    pub state: Option<MidiSyncState>,
}

#[derive(Clone, Debug)]
pub struct MultiSyncDisplay {
    pub state: MultiSyncState,
    pub settings: Settings,
    pub ports: Vec<PortDisplay>,
}

pub enum MultiSyncCommand {
    Start,
    Stop,
    AddListener(Sender<MultiSyncEvent>),
    AddSyncForPort(PortInfo),
    DelSyncForPort(PortInfo),
    UpdateSettings(Settings),
    StartPort(PortInfo),
    StopPort(PortInfo),
}

#[derive(Clone, Debug)]
pub enum MultiSyncEvent {
    Started(Duration),
    Stopped,
    NewPorts(Vec<PortInfo>),
    SettingsUpdated(Settings),
    DisplayUpdate(MultiSyncDisplay),
}

pub struct MultiSyncCtrl {
    listeners: Vec<Sender<MultiSyncEvent>>,
    cmd: Receiver<MultiSyncCommand>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum MultiSyncState {
    Stopped,
    Started(ProgramTime),
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub bpm: f64,
    pub quantum: f64,
    pub tpqn: Option<f64>,
}

pub struct MultiSync {
    ctrl: MultiSyncCtrl,
    port_enum: MidiOutput, // Client used to enumerate available ports
    clients: Vec<MultiSyncMidiClient>,
    settings: Settings,
    state: MultiSyncState,
    changed: bool,
    last_update: Option<ProgramTime>,
    last_port_update: Option<ProgramTime>,
}

#[derive(Clone, PartialEq)]
pub struct PortInfo {
    pub port: MidiOutputPort,
    pub name: String,
}

impl std::fmt::Debug for PortInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(format!("MIDI Output \"{}\"", self.name).as_ref())
    }
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
            .retain(|tx| !matches!(tx.try_send(msg.clone()), Err(TrySendError::Disconnected(_))))
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
                settings: Settings::new(130.0, 16.0, None),
                state: MultiSyncState::Stopped,
                changed: true,
                last_update: None,
                last_port_update: None,
            },
            cmd,
        ))
    }

    pub fn run(&mut self) -> Result<()> {
        self.clients
            .iter_mut()
            .filter_map(|c| c.sync.as_mut())
            .for_each(|s| s.run());

        self.process_cmds().unwrap_or(());
        if self
            .last_port_update
            .and_then(|t| Some(now().0 - t.0 > 1.0.std_seconds()))
            .unwrap_or(true)
        {
            self.update_ports().unwrap_or(());
            self.last_port_update = Some(now());
        }

        let timed_update = self
            .last_update
            .and_then(|t| Some(now().0 - t.0 > 0.5.std_seconds()))
            .unwrap_or(true);

        if self.changed || timed_update {
            self.ctrl
                .publish(MultiSyncEvent::DisplayUpdate(self.to_display()));
            self.last_update = Some(now());
            self.changed = false;
        }

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

        let new_port_info: Vec<PortInfo> = new_ports
            .flat_map(|port| -> Result<PortInfo> {
                let name = self.port_enum.port_name(&port)?;
                info!(port = name, "New Port");
                Ok(PortInfo { port, name })
            })
            .collect();

        self.clients.retain(|p| {
            if ports.contains(&p.info.port) {
                true
            } else {
                self.changed = true;
                false
            }
        });

        let has_new_ports = !new_port_info.is_empty();

        self.clients.extend(
            new_port_info
                .clone()
                .into_iter()
                .map(|p| MultiSyncMidiClient {
                    info: p,
                    sync: None,
                }),
        );

        if has_new_ports {
            self.ctrl.publish(MultiSyncEvent::NewPorts(new_port_info));
            self.changed = true;
        }

        Ok(())
    }

    fn process_cmds(&mut self) -> Result<()> {
        while let Some(cmd) = self.ctrl.get_cmd() {
            self.changed = true;
            let result = match cmd {
                MultiSyncCommand::AddSyncForPort(port) => self.add_sync_for_port(port),
                MultiSyncCommand::DelSyncForPort(port) => self.del_sync_for_port(port),
                MultiSyncCommand::UpdateSettings(settings) => self.update_settings(settings),
                MultiSyncCommand::Start => self.start(),
                MultiSyncCommand::Stop => self.stop(),
                MultiSyncCommand::StartPort(port) => self.start_port(port),
                MultiSyncCommand::StopPort(port) => self.stop_port(port),
                _ => Ok(()),
            };
            if let Err(e) = result {
                error!(error = ?e, "Failed to run command");
            }
        }
        Ok(())
    }

    fn add_sync_for_port(&mut self, port: PortInfo) -> Result<()> {
        let client = self
            .clients
            .iter_mut()
            .find(|p| p.info.port == port.port)
            .context("Port not found")?;
        if client.sync.is_some() {
            bail!(
                "AddSyncForPort: Port already connected to client: {:?}",
                port
            );
        }
        let midi_out = midir::MidiOutput::new("Midimaxe Sync Client")?
            .connect(&client.info.port, "Midimaxe Sync Client Port");
        if let Err(e) = midi_out {
            bail!(
                "AddSyncForPort: Failed to connect to MIDI output: {:?} : {}",
                port,
                e.to_string()
            );
        }
        client.sync = Some(MidiSync::new(
            midi_out.unwrap(),
            self.settings.bpm,
            self.settings.tpqn,
        ));
        info!(port = ?port, "AddSyncForPort: Sync port added");
        Ok(())
    }

    fn del_sync_for_port(&mut self, port: PortInfo) -> Result<()> {
        let client = self
            .clients
            .iter_mut()
            .find(|p| p.info.port == port.port)
            .context("Port not found")?;
        {
            let sync = client.sync.as_ref().context(format!(
                "DelSyncForPort: Port is not connected yet: {:?}",
                port
            ))?;
            match sync.state() {
                MidiSyncState::Stopped | MidiSyncState::Error(_) => (),
                _ => bail!("DelSyncForPort: Port is not stopped: {:?}", port),
            }
        }

        drop(client.sync.take());
        info!(port = ?port, "DelSyncForPort: Sync port removed");

        Ok(())
    }

    fn update_settings(&mut self, settings: Settings) -> Result<()> {
        if let MultiSyncState::Stopped = self.state {
            if !settings.is_valid() {
                warn!(settings = ?settings, "Ignoring invalid settings");
                bail!("UpdateSettings: Invalid Settings {:?}", self.state);
            }
            info!(settings = ?settings, "New settings");
            self.settings = settings;
            self.clients
                .iter_mut()
                .filter_map(|c| c.sync.as_mut())
                .for_each(|s| s.update(self.settings.bpm, self.settings.tpqn).unwrap());
            self.ctrl
                .publish(MultiSyncEvent::SettingsUpdated(self.settings.clone()));
            Ok(())
        } else {
            bail!(
                "UpdateSettings: Cannot update settings in state {:?}",
                self.state
            );
        }
    }

    fn start(&mut self) -> Result<()> {
        let start_time = match self.state {
            MultiSyncState::Stopped => {
                let start_time = ProgramTime(now().0 + 0.1.std_seconds());
                self.state = self.state.transition(MultiSyncState::Started(start_time));
                start_time
            }
            MultiSyncState::Started(start_time) => {
                let next_quantum = self.settings.next_quantum(start_time, None);
                info!(
                    ?start_time,
                    ?next_quantum,
                    "Starting all non-started clients"
                );
                next_quantum
            }
        };
        self.clients
            .iter_mut()
            .filter_map(|c| c.sync.as_mut())
            .for_each(|s| s.start(Some(start_time.0)));

        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        // Send stop command to all clients no matter the current state
        info!("Stopping all clients");
        self.clients
            .iter_mut()
            .filter_map(|c| c.sync.as_mut())
            .for_each(|s| s.stop());
        match self.state {
            MultiSyncState::Started(_) | MultiSyncState::Stopped => {
                self.state = self.state.transition(MultiSyncState::Stopped)
            }
        };
        Ok(())
    }

    fn start_port(&mut self, port: PortInfo) -> Result<()> {
        let start_time = match self.state {
            MultiSyncState::Stopped => {
                bail!(
                    "Cannot start port \"{:?}\" while master is not running",
                    port
                );
            }
            MultiSyncState::Started(start_time) => {
                let next_quantum = self.settings.next_quantum(start_time, None);
                info!(?start_time, ?next_quantum, ?port, "Starting port");
                next_quantum
            }
        };
        match self.clients.iter_mut().find(|p| p.info == port) {
            Some(MultiSyncMidiClient {
                info: _,
                sync: Some(sync),
            }) => {
                info!(?port, ?start_time, "Starting port");
                sync.start(Some(start_time.0));
            }
            Some(_) => bail!("Port has no midisync attached: {:?}", port),

            None => bail!("Port does not exist {:?}", port),
        }

        Ok(())
    }

    fn stop_port(&mut self, port: PortInfo) -> Result<()> {
        match self.clients.iter_mut().find(|p| p.info == port) {
            Some(MultiSyncMidiClient {
                info: _,
                sync: Some(sync),
            }) => {
                info!(?port, "Stopping port");
                sync.stop();
            }
            Some(_) => bail!("Port has no midisync attached: {:?}", port),

            None => bail!("Port does not exist {:?}", port),
        }

        Ok(())
    }

    pub fn to_display(&self) -> MultiSyncDisplay {
        MultiSyncDisplay {
            state: self.state.clone(),
            settings: self.settings.clone(),
            ports: self.clients.iter().map(|c| c.to_display()).collect(),
        }
    }
}

impl MultiSyncState {
    pub fn transition(&self, new_state: MultiSyncState) -> Self {
        if *self != new_state {
            info!(from = ?self, to = ?new_state, "MultiSync state change");
        }
        new_state
    }
}

impl Settings {
    pub fn new(bpm: f64, quantum: f64, tpqn: Option<f64>) -> Self {
        Settings { bpm, quantum, tpqn }
    }

    pub fn next_quantum(&self, start: ProgramTime, current: Option<ProgramTime>) -> ProgramTime {
        let now = current.unwrap_or_else(|| now());
        if now.0 <= start.0 {
            return start;
        }

        let quantum_duration = (60.0 / self.bpm * self.quantum).std_seconds();
        let runtime = now.0 - start.0;
        let quantums = runtime.as_secs_f64() / quantum_duration.as_secs_f64();
        let next_quantum = quantums.ceil();
        ProgramTime(start.0 + (quantum_duration.as_secs_f64() * next_quantum).std_seconds())
    }

    pub fn quantum(&self, start: ProgramTime, current: Option<ProgramTime>) -> f64 {
        let now = current.unwrap_or_else(|| now());
        if now.0 <= start.0 {
            return 0.0;
        }
        let quantum_duration = (60.0 / self.bpm * self.quantum).std_seconds();
        let runtime = now.0 - start.0;
        let quantums = runtime.as_secs_f64() / quantum_duration.as_secs_f64();
        quantums
    }

    pub fn get_quarter(&self, start: ProgramTime, current: Option<ProgramTime>) -> f64 {
        let current = current.unwrap_or_else(|| now());
        if current.0 <= start.0 {
            return 0.0;
        }

        let quarter_duration = (60.0 / self.bpm) as f64;
        let runtime = (current.0 - start.0).as_secs_f64();
        let quarters = runtime / quarter_duration;
        quarters
    }

    pub fn is_valid(&self) -> bool {
        let mut valid = true;

        if self.bpm < 60.0 || self.bpm > 300.0 {
            valid = false;
        }

        if self.quantum < 1.0 {
            valid = false;
        }

        valid
    }
}

impl Drop for MultiSync {
    fn drop(&mut self) {
        self.clients
            .iter_mut()
            .filter_map(|c| c.sync.as_mut())
            .for_each(|s| s.stop());
    }
}

impl MultiSyncMidiClient {
    pub fn to_display(&self) -> PortDisplay {
        PortDisplay {
            info: self.info.clone(),
            state: self.sync.as_ref().and_then(|s| Some(s.state())),
        }
    }
}

impl Default for MultiSyncDisplay {
    fn default() -> Self {
        Self {
            state: MultiSyncState::Stopped,
            settings: Settings {
                bpm: 130.,
                quantum: 4.,
                tpqn: None,
            },
            ports: vec![],
        }
    }
}
