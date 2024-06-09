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

use anyhow::bail;
use midisync::MidiSync;
use multisync::{MultiSyncCommand, MultiSyncEvent, PortInfo, Settings};
use tracing::{debug, error, info, trace, warn};
use utils::programclock;

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
        std::thread::sleep(1.0.std_milliseconds())
    });

    std::thread::sleep(1.0.std_seconds());

    let settings = Settings::new(92.0, 16.0, None);
    info!(settings = ?settings, "Updating settings");
    cmd.send(MultiSyncCommand::UpdateSettings(settings))
        .unwrap();

    info!("Starting!");
    cmd.send(MultiSyncCommand::Start).unwrap();

    loop {
        let msg = listener.recv().unwrap();
        info!(msg = ?msg, "New Message:");
        if let multisync::MultiSyncEvent::NewPorts(p) = msg {
            p.into_iter()
                .filter(|p| p.name.contains("Sync Checker"))
                .for_each(|p| {
                    cmd.send(MultiSyncCommand::AddSyncForPort(p.clone()))
                        .unwrap();
                    cmd.send(MultiSyncCommand::StopPort(p.clone())).unwrap();
                    std::thread::sleep(0.1.std_seconds());
                    cmd.send(MultiSyncCommand::StartPort(p.clone())).unwrap();
                })
        }
    }

    Ok(())
}
