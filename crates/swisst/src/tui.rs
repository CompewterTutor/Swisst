use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};
use std::io;

/// Ratatui multi-select picker. j/k or arrows move, space toggles,
/// enter confirms, q/esc cancels (returns empty vec).
pub fn pick(items: &[String], preselected: &[String]) -> Result<Vec<String>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = ListState::default();
    state.select(Some(0));
    let mut checked = vec![false; items.len()];
    for (i, id) in items.iter().enumerate() {
        if preselected.contains(id) {
            checked[i] = true;
        }
    }

    let result: Result<Vec<String>> = (|| loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(3)])
                .split(f.area());
            let rows: Vec<ListItem> = items
                .iter()
                .enumerate()
                .map(|(i, id)| {
                    let mark = if checked[i] { "[x]" } else { "[ ]" };
                    ListItem::new(format!("{mark} {id}"))
                })
                .collect();
            let list = List::new(rows)
                .block(Block::default().borders(Borders::ALL).title(" pick models (space=toggle, enter=done, q=cancel) "))
                .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
                .highlight_symbol("> ");
            f.render_stateful_widget(list, chunks[0], &mut state);
            let n = checked.iter().filter(|&&c| c).count();
            f.render_widget(
                Paragraph::new(format!("{n}/{len} selected", len = items.len())),
                chunks[1],
            );
        })?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(Vec::new()),
                KeyCode::Enter => {
                    let sel = items
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| checked[*i])
                        .map(|(_, id)| id.clone())
                        .collect();
                    return Ok(sel);
                }
                KeyCode::Char(' ') => {
                    if let Some(i) = state.selected() {
                        checked[i] = !checked[i];
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let i = state.selected().unwrap_or(0);
                    state.select(Some((i + 1).min(items.len().saturating_sub(1))));
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    let i = state.selected().unwrap_or(0);
                    state.select(Some(i.saturating_sub(1)));
                }
                _ => {}
            }
        }
    })();

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}
