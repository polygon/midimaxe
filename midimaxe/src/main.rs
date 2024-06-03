// TODO: Reduce some noise during development, remove these once things are nearing completion
#![allow(unused)] 
#![allow(dead_code)]

use std::io::{stdin, stdout, Write};
use std::thread::sleep;

use midir::{Ignore, MidiInput, MidiOutput, MidiOutputConnection, MidiOutputPort};

use anyhow::{Context, Error, Result};
use midly::live::{LiveEvent, SystemRealtime};
use std::time::Instant;
use time::ext::{InstantExt, NumericalDuration};
use time::Duration;
use std::thread;

mod midisync;
mod multisync;
mod programclock;

use midisync::MidiSync;

fn main() {
    match run() {
        Ok(_) => (),
        Err(err) => println!("Error: {}", err),
    }
}

fn run() -> anyhow::Result<()> {
    let (mut sync, cmd) = multisync::MultiSync::new()?;
    let (s, listener) = crossbeam_channel::unbounded::<multisync::MultiSyncEvent>();

    let t = thread::spawn(move || loop { sync.run(); });

    cmd.send(multisync::MultiSyncCommand::AddListener(s));
    

    Ok(())
}
