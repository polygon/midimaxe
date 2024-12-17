// TODO: Reduce some noise during development, remove these once things are nearing completion
#![allow(unused)]
#![allow(dead_code)]

use anyhow::bail;
use anyhow::Result;
use crossbeam_channel::{unbounded, Receiver, Sender};
use crossterm::{
    event::{self, KeyCode, KeyEvent, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use midir::{os::unix::VirtualInput, MidiInput, MidiInputConnection, MidiInputPort};
use ratatui::layout::Alignment;
use ratatui::layout::Constraint;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::block::Title;
use ratatui::widgets::Padding;
use ratatui::widgets::Widget;
use ratatui::Frame;
use ratatui::{
    prelude::{CrosstermBackend, Layout, Stylize, Terminal},
    widgets::{Block, BorderType, Borders, Paragraph},
};
use std::io::stdout;
use std::time::Duration;
use tracing::{debug, error, info, trace, warn};
use utils::circularbuffer::CircularBuffer;
use utils::midimessages::MidiRealtimeMessage;
use utils::programclock::{now, ProgramTime};

type TimedMessage = (ProgramTime, MidiRealtimeMessage);

#[derive(Debug, Clone)]
pub struct DoubleTime(ProgramTime, Duration);

#[derive(Debug, Clone)]
pub enum ClientState {
    Stopped,
    Started(Option<DoubleTime>),
}

pub struct MidiSyncClient {
    rx: Receiver<TimedMessage>,
    midi_client: MidiInputConnection<()>,
    last_rcv: Option<DoubleTime>,
    history: CircularBuffer<Duration>,
    tpqn: f64,
    state: ClientState,
    total_ticks: f64,
    id: i32,
}

pub struct MidiSyncDisplay {
    state: ClientState,
    id: i32,
    bpm_overall: f64,
    bpm_recent: f64,
    has_clock: bool,
    total_quarters: f64,
}

impl MidiSyncClient {
    pub fn new(history_size: usize, tpqn: f64, id: i32) -> Result<MidiSyncClient> {
        let input = MidiInput::new("Sync Checker")?;
        let (tx, rx) = crossbeam_channel::unbounded::<TimedMessage>();
        let midi_client: MidiInputConnection<()> = input
            .create_virtual(
                format!("Sync Checker Port {}", id).as_ref(),
                move |t, d, p| {
                    if let Some(m) = MidiRealtimeMessage::from_midi(t, d) {
                        tx.send((now(), m)).unwrap_or(())
                    }
                },
                (),
            )
            .or_else(|o| bail!(o.to_string()))?;

        Ok(MidiSyncClient {
            rx,
            midi_client,
            last_rcv: None,
            history: CircularBuffer::new(history_size),
            tpqn,
            state: ClientState::Stopped,
            total_ticks: 0.,
            id,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        while let Ok((time, msg)) = self.rx.try_recv() {
            match (&self.state, msg) {
                (ClientState::Started(Some(_)), MidiRealtimeMessage::MidiClock(t)) => {
                    self.last_rcv = Some(DoubleTime(time, t));
                    self.history.add(t.clone());
                    self.total_ticks += 1.;
                }
                (ClientState::Started(None), MidiRealtimeMessage::MidiClock(t)) => {
                    self.last_rcv = Some(DoubleTime(time, t));
                    self.history.add(t.clone());
                    self.state = ClientState::Started(Some(DoubleTime(time, t)));
                }
                (ClientState::Stopped, MidiRealtimeMessage::MidiStart(t)) => {
                    info!(time = ?DoubleTime(time, t), "Starting...");
                    self.state = ClientState::Started(None);
                    self.history.clear();
                    self.total_ticks = 0.;
                }
                (ClientState::Started(_), MidiRealtimeMessage::MidiStop(t)) => {
                    info!(time = ?DoubleTime(time, t), "Stopping...");
                    self.state = ClientState::Stopped;
                }
                (ClientState::Stopped, MidiRealtimeMessage::MidiStop(t)) => {
                    // Receiving Stop while stopped is valid
                    info!(time = ?DoubleTime(time, t), "Repeated stop...");
                }
                (state, msg) => {
                    warn!(?state, ?msg, "Unexpected message for state")
                }
            }
        }
        Ok(())
    }

    pub fn display(&self) -> MidiSyncDisplay {
        MidiSyncDisplay {
            state: self.state.clone(),
            id: self.id,
            bpm_overall: self.bpm_overall(),
            bpm_recent: self.bpm_recent(),
            has_clock: self
                .last_rcv
                .as_ref()
                .and_then(|lr| Some(now().0 - lr.0 .0 < Duration::from_secs_f64(1.0)))
                .unwrap_or(false),
            total_quarters: (self.total_ticks + 1.) / self.tpqn, // TODO: Incorporate current time here
        }
    }

    fn bpm_overall(&self) -> f64 {
        match &self.state {
            ClientState::Started(Some(t)) => self
                .last_rcv
                .as_ref()
                .and_then(|last_clk| {
                    (last_clk.1 - t.1)
                        .as_nanos()
                        .checked_div(self.total_ticks as u128)
                        .and_then(|t_tick| {
                            let t_beat = t_tick as f64 * self.tpqn / 1000000000.;
                            Some(60.0 / t_beat)
                        })
                })
                .unwrap_or(0.),
            _ => 0.,
        }
    }

    fn bpm_recent(&self) -> f64 {
        if self.history.get_buf().len() < 2 {
            return 0.;
        }

        let t_total = self.history.get_buf().back().unwrap().as_secs_f64()
            - self.history.get_buf().front().unwrap().as_secs_f64();
        let t_beat = t_total / (self.history.get_buf().len() - 1) as f64 * self.tpqn;
        60. / t_beat
    }
}

fn test_sync() -> Result<()> {
    tracing_subscriber::fmt()
        .with_thread_ids(true)
        .with_file(true)
        .with_level(true)
        .with_line_number(true)
        .init();

    let mut sync1 = MidiSyncClient::new(100, 24.0, 1).unwrap();
    let mut sync2 = MidiSyncClient::new(100, 24.0, 2).unwrap();

    loop {
        sync1.run().unwrap();
        sync2.run().unwrap();
        sync1.bpm_overall();

        std::thread::sleep(std::time::Duration::from_secs_f64(0.001))
    }

    Ok(())
}

pub struct SyncClient {
    clients: Vec<MidiSyncClient>,
    selected: usize,
}

impl Widget for &SyncClient {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let layout = Layout::vertical((0..self.clients.len()).map(|_| Constraint::Max(8)));
        let client_areas = layout.split(area);

        self.clients.iter().enumerate().for_each(|(i, cl)| {
            let disp = cl.display();
            let widget = SyncClientWidget {
                disp,
                selected: i == self.selected,
            };
            widget.render(client_areas[i], buf);
        });
    }
}

fn main() -> Result<()> {
    //test_sync()?;
    let mut sc = SyncClient {
        clients: vec![],
        selected: 0,
    };
    let mut client_id = 2;
    sc.clients.push(MidiSyncClient::new(10, 24.0, 1).unwrap());
    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;

    loop {
        sc.clients.iter_mut().for_each(|c| c.run().unwrap());
        terminal.draw(|frame| {
            render(&sc, frame);
        })?;

        if event::poll(std::time::Duration::from_millis(16))? {
            if let event::Event::Key(key) = event::read()? {
                match (key.kind, key.code) {
                    (KeyEventKind::Press, KeyCode::Char('q')) => break,
                    (KeyEventKind::Press, KeyCode::Char('+')) => {
                        sc.clients
                            .push(MidiSyncClient::new(10, 24.0, client_id).unwrap());
                        client_id += 1;
                    }
                    (KeyEventKind::Press, KeyCode::Up) => {
                        if sc.selected > 0 {
                            sc.selected -= 1;
                        }
                        if sc.selected >= sc.clients.len() && !sc.clients.is_empty() {
                            sc.selected = sc.clients.len() - 1;
                        }
                    }
                    (KeyEventKind::Press, KeyCode::Down) => {
                        if !sc.clients.is_empty() {
                            if sc.selected < sc.clients.len() - 1 {
                                sc.selected += 1;
                            }
                        }
                        if sc.selected >= sc.clients.len() && !sc.clients.is_empty() {
                            sc.selected = sc.clients.len() - 1;
                        }
                    }
                    (KeyEventKind::Press, KeyCode::Delete) => {
                        if !sc.clients.is_empty() {
                            sc.clients.remove(sc.selected);
                            if sc.clients.is_empty() {
                                sc.selected = 0;
                            } else if sc.selected >= sc.clients.len() {
                                sc.selected = sc.clients.len() - 1;
                            }
                        }
                    }
                    _ => (),
                }
            }
        }
    }

    stdout().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}

pub fn render(sc: &SyncClient, frame: &mut Frame) {
    let area = frame.size();

    let widgets = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]);
    let widget_areas: [Rect; 2] = widgets.areas(area);

    frame.render_widget(sc, *widget_areas.first().unwrap());
    frame.render_widget(
        Paragraph::new(" (+) Add client, (del) Remove Client, (up/down) Select, (q) Exit"),
        *widget_areas.last().unwrap(),
    );
}

struct SyncClientWidget {
    disp: MidiSyncDisplay,
    selected: bool,
}
struct BeatWidget(f64);

impl Widget for BeatWidget {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let layout = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]);
        let areas: [Rect; 2] = layout.areas(area);

        let active_sixteenth = ((self.0 / 4.).fract() * 16.).floor() as usize;
        let mut quantum = Paragraph::new(Line::from(
            "1 e & a 2 e & a 3 e & a 4 e & a "
                .chars()
                .enumerate()
                .map(|(i, c)| {
                    if (i % 2) == 1 {
                        return " ".hidden();
                    }
                    let i = i / 2;
                    let mut base = if (i % 4) == 0 || (i == active_sixteenth) {
                        format!("{}", c)
                    } else {
                        format!(" ")
                    };

                    if i == active_sixteenth {
                        base.black().bold().on_white()
                    } else if (i % 4) == 0
                        && (active_sixteenth as i32 - i as i32) < 4
                        && (active_sixteenth as i32 - i as i32) >= 0
                    {
                        base.green().bold()
                    } else {
                        base.gray().dim()
                    }
                })
                .collect::<Vec<Span>>(),
        ));
        let bars_prog = ((self.0 * 2.) % 32.0 + 1.) as usize;
        let mut bars = Paragraph::new((0..bars_prog).map(|_| " ").collect::<String>().on_white());

        quantum.render(areas[0], buf);
        bars.render(areas[1], buf);
    }
}

impl Widget for SyncClientWidget {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let display = self.disp;
        let mut outer = Block::bordered()
            .title_top(format!(" Sync Client {} ", display.id))
            .padding(Padding::uniform(1));
        if self.selected {
            outer = outer.red();
        }
        match (&display.state, &display.has_clock) {
            (ClientState::Stopped, _) => {
                outer = outer.title_top(Line::from(" STOPPED ").left_aligned().red().bold());
            }
            (ClientState::Started(_), true) => {
                outer = outer.title_top(Line::from(" STARTED ").left_aligned().green().bold());
            }
            (ClientState::Started(_), false) => {
                outer = outer.title_top(
                    Line::from(vec![
                        " STARTED ".green().into(),
                        " - ".into(),
                        "(NO CLK) ".red().bold(),
                    ])
                    .left_aligned()
                    .bold(),
                );
            }
        }

        let canvas_area = outer.inner(area);
        outer.render(area, buf);

        let canvas_layout = Layout::vertical([Constraint::Length(3), Constraint::Length(1)]);
        let canvas_areas: [Rect; 2] = canvas_layout.areas(canvas_area);

        let beats = BeatWidget(display.total_quarters);

        let mut bpmline = Paragraph::new(Line::from(vec![
            "BPM overall / recent - ".into(),
            format!("{:>5.1}", display.bpm_overall).bold(),
            " / ".into(),
            format!("{:<5.1}", display.bpm_recent).bold(),
        ]));
        beats.render(canvas_areas[0], buf);
        bpmline.render(canvas_areas[1], buf);
    }
}
