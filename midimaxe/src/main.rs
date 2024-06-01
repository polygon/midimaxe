use std::io::{stdin, stdout, Write};
use std::thread::sleep;

use midir::{Ignore, MidiInput, MidiOutput, MidiOutputConnection, MidiOutputPort};

use anyhow::{Context, Error, Result};
use midly::live::{LiveEvent, SystemRealtime};
use std::time::Instant;
use time::ext::{InstantExt, NumericalDuration};
use time::Duration;

mod midisync;
mod multisync;

use midisync::MidiSync;

fn main() {
    match run() {
        Ok(_) => (),
        Err(err) => println!("Error: {}", err),
    }
}

fn run() -> anyhow::Result<()> {
    let midi_out = MidiOutput::new("midir test output")?;

    let ports = midi_out.ports();
    let virtual_ports: Vec<_> = ports
        .iter()
        .filter(|port| {
            midi_out
                .port_name(port)
                .is_ok_and(|name| name.contains("Virtual"))
        })
        .collect();
    println!("\nAvailable output ports:");
    for (i, p) in virtual_ports.iter().enumerate() {
        println!("{}: {}", i, midi_out.port_name(p)?);
    }

    if virtual_ports.is_empty() {
        println!("No virtual port found");
        return Ok(());
    }

    println!("Connecting to first Virtual Output port");

    let sync_port = midi_out.connect(virtual_ports[0], "sync").unwrap();

    let oe = MidiOutput::new("midir test output2")?;
    oe.ports()
        .iter()
        .enumerate()
        .for_each(|(i, p)| println!("{}: {}", i, oe.port_name(p).unwrap_or("FAILED".to_owned())));

    let bpm1: f64 = 183.3;
    let mut sync = MidiSync::new(sync_port, bpm1, None);
    let t_start = Instant::now();
    sync.start(None);
    loop {
        sync.run();
        if t_start.elapsed() > 10.seconds() {
            break;
        }
    }
    sync.stop();

    Ok(())
}
