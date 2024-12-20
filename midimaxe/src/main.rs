use std::io::stdout;

use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;

use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::thread;
use time::ext::NumericalStdDuration;

mod midisync;
mod multisync;
mod ui;

use multisync::MultiSyncCommand;
use ui::MultiSyncUi;
use utils::programclock::{self, now};

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
    cmd.send(MultiSyncCommand::AddListener(s)).unwrap();
    let mut ui = MultiSyncUi::new(cmd, listener);

    let _t = thread::spawn(move || loop {
        let sleep_duration = sync
            .run()
            .unwrap_or(None)
            .and_then(|next_event| {
                let current = now().0;
                if current < next_event {
                    Some(next_event - current)
                } else {
                    None
                }
            })
            .unwrap_or(10.0.std_milliseconds());
        if sleep_duration > 0.0.std_milliseconds() {
            std::thread::sleep(sleep_duration);
        }
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
