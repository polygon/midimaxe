// TODO: Reduce some noise during development, remove these once things are nearing completion
#![allow(unused)]
#![allow(dead_code)]

use anyhow::bail;
use anyhow::Result;
use crossterm::{
    event::{self, KeyCode, KeyEvent, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
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

fn main() -> Result<()> {
    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;

    let mut evs: Vec<String> = Vec::new();

    loop {
        terminal.draw(|frame| {
            let area = frame.size();
            let inner_text: Vec<Line> =
                evs.iter().map(|s| Line::from(vec![Span::raw(s)])).collect();

            let mut p = Paragraph::new(inner_text);
            frame.render_widget(p, area);
        })?;

        if event::poll(std::time::Duration::from_millis(16))? {
            let event = event::read()?;
            evs.push(format!("{:?}", event));
            if let event::Event::Key(key) = event {
                match (key.kind, key.code) {
                    (KeyEventKind::Press, KeyCode::Char('q')) => break,
                    _ => (),
                }
            }
        }

        while (evs.len() > 10) {
            evs.remove(0);
        }
    }

    stdout().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}
