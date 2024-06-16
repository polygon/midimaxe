// TODO: Reduce some noise during development, remove these once things are nearing completion
#![allow(unused)]
#![allow(dead_code)]

use std::io::{stdin, stdout, Write};
use std::thread::sleep;

use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use midir::{Ignore, MidiInput, MidiOutput, MidiOutputConnection, MidiOutputPort};

use anyhow::{Context, Error, Result};
use midly::live::{LiveEvent, SystemRealtime};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::thread;
use std::time::Instant;
use time::ext::{InstantExt, NumericalStdDuration};
use time::Duration;

mod midisync;
mod multisync;
mod ui;

use anyhow::bail;
use midisync::MidiSync;
use multisync::{MultiSyncCommand, MultiSyncEvent, PortInfo, Settings};
use tracing::{debug, error, info, trace, warn};
use ui::MultiSyncUi;
use utils::programclock;

fn main() {
    /*tracing_subscriber::fmt()
    .with_thread_ids(true)
    .with_file(true)
    .with_level(true)
    .with_line_number(true)
    .init();*/
    match run() {
        Ok(_) => (),
        Err(err) => println!("Error: {}", err),
    }
}

fn run() -> anyhow::Result<()> {
    programclock::now();
    let (mut sync, cmd) = multisync::MultiSync::new()?;
    let (s, listener) = crossbeam_channel::unbounded::<multisync::MultiSyncEvent>();
    cmd.send(MultiSyncCommand::AddListener(s));
    let mut ui = MultiSyncUi::new(cmd, listener);

    let t = thread::spawn(move || loop {
        sync.run();
        std::thread::sleep(1.0.std_milliseconds())
    });

    // Initialize console
    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;

    loop {
        ui.update();

        terminal.draw(|frame| {
            frame.render_widget(&mut ui, frame.size());
        })?;

        if ui.exit_requested {
            break;
        }

        std::thread::sleep(16.7.std_milliseconds());
    }

    stdout().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;

    Ok(())
}
