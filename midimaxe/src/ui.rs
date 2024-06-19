use crate::midisync::MidiSyncState;
use crate::multisync::MultiSyncState;
use crate::multisync::{MultiSyncCommand, MultiSyncDisplay, MultiSyncEvent, Settings};
use crossbeam_channel::{Receiver, Sender};
use crossterm::event::{self, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Constraint;
use ratatui::layout::Rect;
use ratatui::layout::{Alignment, Direction};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::{Cell, Padding, Table, TableState};
use ratatui::widgets::{Row, StatefulWidget, Widget};
use ratatui::{
    prelude::{Layout, Stylize},
    widgets::{Block, Clear, Paragraph},
};
use std::f64::consts::PI;
use time::ext::NumericalDuration;
use tui_big_text::{BigText, BigTextBuilder, PixelSize};
use utils::programclock::{now, ProgramTime};

pub struct MultiSyncUi {
    pub exit_requested: bool,
    cmd: Sender<MultiSyncCommand>,
    recv: Receiver<MultiSyncEvent>,
    first_exit: Option<ProgramTime>,
    first_stop: Option<ProgramTime>,
    disp: MultiSyncDisplay,
    table_state: TableState,
}

impl Widget for &mut MultiSyncUi {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let layout = Layout::vertical(vec![
            Constraint::Length(12),
            Constraint::Min(1),
            Constraint::Length(1),
        ]);
        let areas: [Rect; 3] = layout.areas(area);

        CommonArea(&self.disp).render(areas[0], buf);
        ClientArea(&self.disp, &mut self.table_state).render(areas[1], buf);
        ExitConfirmation(self.first_exit, "Press Ctrl+C again to exit".to_owned())
            .render(area, buf);
        ExitConfirmation(self.first_stop, "Press Shift+Z again to stop".to_owned())
            .render(area, buf);
    }
}

struct ExitConfirmation(Option<ProgramTime>, String);
struct CommonArea<'a>(&'a MultiSyncDisplay);
struct ClientArea<'a>(&'a MultiSyncDisplay, &'a mut TableState);
struct BeatLine<'a>(&'a MultiSyncDisplay);

impl Widget for ExitConfirmation {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        if let Some(t) = self.0 {
            if now().0 - t.0 > 1.0.seconds() {
                return;
            }
        } else {
            return;
        }

        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((50) / 2),
                Constraint::Length(5),
                Constraint::Percentage((50) / 2),
            ])
            .split(area);

        let parea = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(40 / 2),
                Constraint::Percentage(60),
                Constraint::Percentage(40 / 2),
            ])
            .split(popup_layout[1])[1];

        let msg = Block::bordered()
            .padding(Padding::uniform(1))
            .style(Style::new().on_red().white())
            .title("Confirm".bold());

        let inner = msg.inner(parea);
        Clear.render(parea, buf);
        msg.render(parea, buf);

        let cnf = Paragraph::new(self.1)
            .bold()
            .alignment(Alignment::Center)
            .white();
        cnf.render(inner, buf);
    }
}

impl<'a> Widget for CommonArea<'a> {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let mut block = Block::bordered().padding(Padding::uniform(1));
        match self.0.state {
            MultiSyncState::Stopped => {
                block = block.title(" STOPPED ".slow_blink().red().bold());
                block =
                    block.title_bottom(" (Shift+s) Start, (Shift+z) Stop all, ([Shift] left/right) BPM, (</>) Quantum ")
            }
            MultiSyncState::Started(_) => {
                block = block.title(" RUNNING ".green().bold());
                block = block.title_bottom(" (Shift+s) Start all, (Shift+z) Stop all ")
            }
        }

        let inner = block.inner(area);
        block.render(area, buf);

        let inner_layout =
            Layout::vertical([Constraint::Length(3), Constraint::Length(4)]).spacing(1);
        let inner_area: [Rect; 2] = inner_layout.areas(inner);

        let inner_text = vec![
            Line::from(vec![
                Span::styled(
                    format!("{:>5.1} ", self.0.settings.bpm),
                    Style::new().white().bold(),
                ),
                Span::raw("BPM    "),
                match self.0.state {
                    MultiSyncState::Started(since) => {
                        let running = now().0.as_secs_f64() - since.0.as_secs_f64();
                        Span::styled(
                            format!(
                                "{:02}:{:02}:{:07.4}",
                                (running / 3600.).floor(),
                                ((running / 60.0) % 60.0).floor(),
                                (running % 60.0)
                            ),
                            Style::new(),
                        )
                    }
                    _ => Span::styled("00:00:00.0000", Style::new().slow_blink()),
                },
            ]),
            Line::from(vec![]),
            Line::from(vec![Span::raw(format!(
                "Quantum {:2}",
                self.0.settings.quantum
            ))]),
        ];
        let ip = Paragraph::new(inner_text);
        let beatline = BeatLine(&self.0);
        ip.render(inner_area[0], buf);
        beatline.render(inner_area[1], buf);
    }
}

impl<'a> Widget for ClientArea<'a> {
    fn render(mut self, area: Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let block = Block::bordered()
            .padding(Padding::uniform(1))
            .title(" Clients ")
            .title_bottom(" (Up/Down) Select, (Enter) Add/Start, (z) Stop, (Del) Remove ");

        let inner = block.inner(area);
        block.render(area, buf);

        let rows: Vec<Row> = self
            .0
            .ports
            .iter()
            .map(|port| {
                Row::new(vec![{
                    let style = match port.state {
                        Some(MidiSyncState::Running) => Style::new().green(),
                        Some(MidiSyncState::Stopped) => Style::new().white(),
                        Some(MidiSyncState::Starting) => Style::new().yellow().dim().slow_blink(),
                        _ => Style::default(),
                    };
                    Cell::new(port.info.name.to_owned()).style(style)
                }])
            })
            .collect();

        let clients = Table::new(rows, [Constraint::Min(40)])
            .highlight_style(Style::new().reversed())
            // ...and potentially show a symbol in front of the selection.
            .highlight_symbol(" >> ");
        let table_state: &mut TableState = &mut self.1;
        StatefulWidget::render(clients, inner, buf, table_state);
    }
}

impl<'a> Widget for BeatLine<'a> {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let t = now();
        let (intensity, last_4, fill, mfill) = if let MultiSyncState::Started(ts) = self.0.state {
            let quarter = self.0.settings.get_quarter(ts, Some(t));
            let partial = quarter.fract();
            let beat_prog = partial / 0.5;
            let intensity = if beat_prog < 1.0 {
                (PI / 2.0 * beat_prog).cos().powf(0.5)
            } else {
                0.0
            };
            let last_4 = quarter.floor() as u8 % 4;
            let bars_completed = (quarter / 4.0).floor();
            let total_bars_in_quantum = (self.0.settings.quantum / 4.0).ceil();
            let bars_in_quantum_completed = bars_completed % total_bars_in_quantum;
            let fill = (16.0 * bars_in_quantum_completed / total_bars_in_quantum) as u32;
            let mut mfill = (16.0 / total_bars_in_quantum) as u32;
            if fill + mfill == 15 && mfill > 1 {
                mfill += 1;
            }
            ((255.0 * intensity) as u8, last_4 + 1, fill, mfill)
        } else {
            (0, 1, 0, 0)
        };
        let bl = BigText::builder()
            .pixel_size(PixelSize::Quadrant)
            .lines(vec![Line::from(vec![
                Span::styled(" ", Style::new().bg(Color::Rgb(intensity, 0, 0))),
                Span::raw(format!(" {} ", last_4)),
                Span::styled(
                    (0..fill).map(|_| " ").collect::<String>(),
                    Style::new().bg(Color::Rgb(255, 255, 255)),
                ),
                Span::styled(
                    (0..mfill).map(|_| " ").collect::<String>(),
                    Style::new().bg(Color::Rgb(128, 128, 128)),
                ),
                Span::styled(
                    (0..(16 - fill - mfill)).map(|_| " ").collect::<String>(),
                    Style::new().bg(Color::Rgb(0, 0, 0)),
                ),
            ])])
            .build()
            .unwrap();
        bl.render(area, buf);
    }
}

impl MultiSyncUi {
    pub fn new(cmd: Sender<MultiSyncCommand>, recv: Receiver<MultiSyncEvent>) -> MultiSyncUi {
        MultiSyncUi {
            exit_requested: false,
            first_exit: None,
            first_stop: None,
            cmd,
            recv,
            disp: MultiSyncDisplay::default(),
            table_state: TableState::default().with_selected(Some(0)),
        }
    }
    pub fn update(&mut self) {
        self.process_key_events();
        self.process_sync_events();
    }

    fn process_key_events(&mut self) {
        if event::poll(std::time::Duration::from_millis(16)).unwrap() {
            if let event::Event::Key(key) = event::read().unwrap() {
                match (key.kind, key.code, key.modifiers) {
                    (KeyEventKind::Press, KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                        self.exit_requested = self.request_quit();
                    }
                    (
                        KeyEventKind::Press | KeyEventKind::Repeat,
                        KeyCode::Left | KeyCode::Right,
                        KeyModifiers::NONE | KeyModifiers::SHIFT,
                    ) => {
                        self.control_bpm(key);
                    }
                    (
                        KeyEventKind::Press | KeyEventKind::Repeat,
                        KeyCode::Up | KeyCode::Down,
                        _,
                    ) => {
                        self.navigate(key);
                    }
                    (KeyEventKind::Press, KeyCode::Enter, KeyModifiers::NONE) => {
                        self.act_on_port();
                    }
                    (KeyEventKind::Press, KeyCode::Char('z'), KeyModifiers::NONE) => {
                        self.stop_port();
                    }
                    (KeyEventKind::Press, KeyCode::Char('S'), KeyModifiers::SHIFT) => {
                        self.start_all();
                    }
                    (KeyEventKind::Press, KeyCode::Char('Z'), KeyModifiers::SHIFT) => {
                        self.request_stop_all();
                    }
                    (KeyEventKind::Press, KeyCode::Char('>'), KeyModifiers::NONE) => {
                        self.control_quantum(true);
                    }
                    (KeyEventKind::Press, KeyCode::Char('<'), KeyModifiers::NONE) => {
                        self.control_quantum(false);
                    }
                    (KeyEventKind::Press, KeyCode::Delete, KeyModifiers::NONE) => {
                        self.remove_port();
                    }
                    _ => (),
                }
            }
        }
    }

    fn process_sync_events(&mut self) {
        while let Ok(msg) = self.recv.try_recv() {
            match msg {
                MultiSyncEvent::DisplayUpdate(disp) => {
                    self.disp = disp;
                    self.post_update_checks();
                }
                _ => (),
            }
        }
    }

    fn request_quit(&mut self) -> bool {
        if let Some(t) = self.first_exit {
            if now().0 - t.0 < 1.0.seconds() {
                return true;
            }
        }

        self.first_exit = Some(now());

        false
    }

    fn control_bpm(&mut self, key: KeyEvent) {
        let dir = if let KeyCode::Right = key.code {
            1.0
        } else {
            -1.0
        };

        let amt = match key.modifiers {
            KeyModifiers::NONE => 1.0,
            KeyModifiers::SHIFT => 10.0,
            _ => 0.0,
        };

        let total: f64 = dir * amt;
        if total.abs() > 0.01 {
            self.cmd
                .send(MultiSyncCommand::UpdateSettings(Settings {
                    bpm: self.disp.settings.bpm + total,
                    ..self.disp.settings
                }))
                .unwrap();
        }
    }

    fn control_quantum(&mut self, inc: bool) {
        let nc = if inc {
            ((self.disp.settings.quantum / 4.0).floor() + 1.0) * 4.0
        } else {
            ((self.disp.settings.quantum / 4.0).floor() - 1.0) * 4.0
        };

        self.cmd
            .send(MultiSyncCommand::UpdateSettings(Settings {
                quantum: nc,
                ..self.disp.settings
            }))
            .unwrap();
    }

    fn navigate(&mut self, key: KeyEvent) {
        let newidx = match (key.code, self.table_state.selected()) {
            (_, None) => Some(0),
            (KeyCode::Up, Some(0)) => Some(self.disp.ports.len() - 1),
            (KeyCode::Up, Some(i)) => Some(i - 1),
            (KeyCode::Down, Some(i)) => {
                if i == self.disp.ports.len() - 1 {
                    Some(0)
                } else {
                    Some(i + 1)
                }
            }
            (_, Some(i)) => Some(i),
        };

        self.table_state = self.table_state.clone().with_selected(newidx);
    }

    fn post_update_checks(&mut self) {
        let new_state = match self.table_state.selected() {
            Some(i) => {
                if i >= self.disp.ports.len() && self.disp.ports.len() > 0 {
                    Some(self.disp.ports.len() - 1)
                } else {
                    Some(i)
                }
            }
            None => Some(0),
        };

        self.table_state = self.table_state.clone().with_selected(new_state);
    }

    fn act_on_port(&mut self) {
        if let Some(idx) = self.table_state.selected() {
            if let Some(port) = self.disp.ports.get(idx) {
                match port.state {
                    None => {
                        self.cmd
                            .send(MultiSyncCommand::AddSyncForPort(port.info.clone()))
                            .unwrap();
                    }
                    Some(MidiSyncState::Stopped) => {
                        self.cmd
                            .send(MultiSyncCommand::StartPort(port.info.clone()))
                            .unwrap();
                    }
                    _ => (),
                }
            }
        }
    }

    fn stop_port(&mut self) {
        if let Some(idx) = self.table_state.selected() {
            if let Some(port) = self.disp.ports.get(idx) {
                self.cmd
                    .send(MultiSyncCommand::StopPort(port.info.clone()))
                    .unwrap();
            }
        }
    }

    fn remove_port(&mut self) {
        if let Some(idx) = self.table_state.selected() {
            if let Some(port) = self.disp.ports.get(idx) {
                self.cmd
                    .send(MultiSyncCommand::DelSyncForPort(port.info.clone()))
                    .unwrap();
            }
        }
    }

    fn start_all(&mut self) {
        self.cmd.send(MultiSyncCommand::Start).unwrap();
    }

    fn request_stop_all(&mut self) {
        if let MultiSyncState::Stopped = self.disp.state {
            self.cmd.send(MultiSyncCommand::Stop).unwrap();
            return;
        }
        if let Some(t) = self.first_stop {
            if now().0 - t.0 < 1.0.seconds() {
                self.first_stop = None;
                self.cmd.send(MultiSyncCommand::Stop).unwrap();
                return;
            }
        }

        self.first_stop = Some(now());
    }
}
