// TODO: Reduce some noise during development, remove these once things are nearing completion
#![allow(unused)]
#![allow(dead_code)]

use std::io::{stdin, stdout, Write};
use std::thread::sleep;

use midir::{Ignore, MidiInput, MidiOutput, MidiOutputConnection, MidiOutputPort};

use anyhow::{Context, Error, Result};
use midly::live::{LiveEvent, SystemRealtime};
use std::thread;
use std::time::Instant;
use time::ext::{InstantExt, NumericalStdDuration};
use time::Duration;

mod midisync;
mod multisync;
mod programclock;

use anyhow::bail;
use midisync::MidiSync;
use multisync::{MultiSyncCommand, MultiSyncEvent, PortInfo, Settings};
use tracing::{debug, error, info, trace, warn};

fn main() {
    tracing_subscriber::fmt()
        .with_thread_ids(true)
        .with_file(true)
        .with_level(true)
        .with_line_number(true)
        .init();
    match run() {
        Ok(_) => (),
        Err(err) => println!("Error: {}", err),
    }
}

fn run() -> anyhow::Result<()> {
    programclock::now();
    let (mut sync, cmd) = multisync::MultiSync::new()?;
    let (s, listener) = crossbeam_channel::unbounded::<multisync::MultiSyncEvent>();

    cmd.send(multisync::MultiSyncCommand::AddListener(s));
    let t = thread::spawn(move || loop {
        sync.run();
    });

    std::thread::sleep(1.0.std_seconds());

    let msg = listener.recv().unwrap();
    info!(msg = ?msg, "New Message:");
    let ports = match msg {
        multisync::MultiSyncEvent::NewPorts(p) => p,
        _ => bail!("Wrong message"),
    };
    let virtual_ports: Vec<PortInfo> = ports
        .into_iter()
        .filter(|p| p.name.contains("Virtual"))
        .collect();
    if virtual_ports.is_empty() {
        bail!("No virtual ports found")
    }

    std::thread::sleep(0.2.std_seconds());

    let settings = Settings::new(110.0, 16.0, None);
    info!(settings = ?settings, "Updating settings");
    cmd.send(MultiSyncCommand::UpdateSettings(settings))
        .unwrap();

    let msg = listener.recv().unwrap();
    info!(msg = ?msg, "New Message:");
    match msg {
        multisync::MultiSyncEvent::SettingsUpdated(s) => (),
        _ => bail!("Wrong message"),
    };

    info!("Starting!");
    cmd.send(MultiSyncCommand::Start).unwrap();

    std::thread::sleep(0.2.std_seconds());

    info!(port = virtual_ports[0].name, "Adding virtual port");

    cmd.send(MultiSyncCommand::AddSyncForPort(virtual_ports[0].clone()))
        .unwrap();
    cmd.send(MultiSyncCommand::AddSyncForPort(virtual_ports[0].clone()))
        .unwrap();

    std::thread::sleep(3.0.std_seconds());

    info!("Starting port!");
    cmd.send(MultiSyncCommand::StartPort(virtual_ports[0].clone()))
        .unwrap();

    std::thread::sleep(40.std_seconds());

    cmd.send(MultiSyncCommand::Stop).unwrap();

    std::thread::sleep(0.2.std_seconds());

    Ok(())
}
