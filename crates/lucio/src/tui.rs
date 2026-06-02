//! Interactive "carry-over" category selector (ratatui).
//!
//! The selection logic is a pure state machine ([`Selection`]) so it can be
//! unit-tested without a terminal; [`select_categories`] is the thin terminal
//! wrapper. `ratatui::init`/`restore` install a panic hook that restores the
//! terminal, and we also restore on the normal/`Err` return paths.

use anyhow::Result;
use lucio_core::manifest::CATEGORIES;
use ratatui::DefaultTerminal;
use ratatui::Frame;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};

/// Run the interactive selector. Returns the chosen category ids on confirm, or
/// `None` if the user cancelled.
///
/// # Errors
/// Propagates terminal setup / draw / input errors.
pub fn select_categories() -> Result<Option<Vec<String>>> {
    let mut terminal = ratatui::try_init()?;
    let outcome = run(&mut terminal);
    let _ = ratatui::try_restore();
    outcome
}

/// The terminal event loop, separated so the terminal is always restored.
fn run(terminal: &mut DefaultTerminal) -> Result<Option<Vec<String>>> {
    let mut selection = Selection::new();
    loop {
        terminal.draw(|frame| render(frame, &selection))?;
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match selection.handle_key(key.code) {
                Action::Continue => {}
                Action::Confirm => return Ok(Some(selection.selected_ids())),
                Action::Cancel => return Ok(None),
            }
        }
    }
}

/// What a key press should do.
#[derive(Debug)]
enum Action {
    Continue,
    Confirm,
    Cancel,
}

/// Pure selection state over [`CATEGORIES`] (cursor + per-row checked flags).
#[derive(Debug)]
struct Selection {
    cursor: usize,
    checked: Vec<bool>,
}

impl Selection {
    fn new() -> Self {
        Self {
            cursor: 0,
            checked: CATEGORIES.iter().map(|cat| cat.default_on).collect(),
        }
    }

    const fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    const fn move_down(&mut self) {
        if self.cursor + 1 < CATEGORIES.len() {
            self.cursor += 1;
        }
    }

    fn toggle(&mut self) {
        if let Some(flag) = self.checked.get_mut(self.cursor) {
            *flag = !*flag;
        }
    }

    /// Check everything, or clear everything if all are already checked.
    fn toggle_all(&mut self) {
        let all = self.checked.iter().all(|&c| c);
        for flag in &mut self.checked {
            *flag = !all;
        }
    }

    /// Restore the preselected (default) set.
    fn reset_defaults(&mut self) {
        for (flag, cat) in self.checked.iter_mut().zip(CATEGORIES) {
            *flag = cat.default_on;
        }
    }

    fn selected_ids(&self) -> Vec<String> {
        CATEGORIES
            .iter()
            .zip(&self.checked)
            .filter(|&(_, &checked)| checked)
            .map(|(cat, _)| cat.id.to_owned())
            .collect()
    }

    fn handle_key(&mut self, code: KeyCode) -> Action {
        match code {
            KeyCode::Up | KeyCode::Char('k') => self.move_up(),
            KeyCode::Down | KeyCode::Char('j') => self.move_down(),
            KeyCode::Char(' ') => self.toggle(),
            KeyCode::Char('a') => self.toggle_all(),
            KeyCode::Char('d') => self.reset_defaults(),
            KeyCode::Enter => return Action::Confirm,
            KeyCode::Esc | KeyCode::Char('q') => return Action::Cancel,
            _ => {}
        }
        Action::Continue
    }
}

/// Draw the checklist and footer.
fn render(frame: &mut Frame, selection: &Selection) {
    let layout = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(frame.area());

    let rows: Vec<ListItem> = CATEGORIES
        .iter()
        .zip(&selection.checked)
        .map(|(cat, &checked)| {
            let mark = if checked { "[x]" } else { "[ ]" };
            let mut spans = vec![Span::raw(format!("{mark} {}", cat.label))];
            if cat.sensitive {
                spans.push(Span::styled(
                    "  · personal data",
                    Style::new().fg(Color::Yellow),
                ));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(rows)
        .block(Block::bordered().title(" Carry over to the new profile "))
        .highlight_symbol("➤ ")
        .highlight_style(Style::new().add_modifier(Modifier::BOLD));
    let mut state = ListState::default();
    state.select(Some(selection.cursor));
    frame.render_stateful_widget(list, layout[0], &mut state);

    let footer =
        Paragraph::new("↑/↓ move · space toggle · a all · d defaults · enter create · esc cancel")
            .style(Style::new().fg(Color::DarkGray));
    frame.render_widget(footer, layout[1]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use lucio_core::manifest::default_category_ids;

    fn default_ids() -> Vec<String> {
        default_category_ids()
            .iter()
            .map(|id| (*id).to_owned())
            .collect()
    }

    #[test]
    fn starts_with_defaults_checked() {
        assert_eq!(Selection::new().selected_ids(), default_ids());
    }

    #[test]
    fn space_toggles_the_cursor_row_off() {
        let mut s = Selection::new();
        // Cursor starts on row 0 ("settings", checked) → toggle it off.
        assert!(matches!(s.handle_key(KeyCode::Char(' ')), Action::Continue));
        assert!(!s.selected_ids().contains(&"settings".to_owned()));
    }

    #[test]
    fn navigating_down_and_toggling_enables_an_extra() {
        let mut s = Selection::new();
        // Rows: 0 settings, 1 extensions, 2 extension-options, 3 bookmarks.
        s.handle_key(KeyCode::Down);
        s.handle_key(KeyCode::Down);
        s.handle_key(KeyCode::Down);
        s.handle_key(KeyCode::Char(' '));
        assert!(s.selected_ids().contains(&"bookmarks".to_owned()));
    }

    #[test]
    fn enter_confirms_and_esc_cancels() {
        let mut s = Selection::new();
        assert!(matches!(s.handle_key(KeyCode::Enter), Action::Confirm));
        assert!(matches!(s.handle_key(KeyCode::Esc), Action::Cancel));
    }

    #[test]
    fn toggle_all_then_reset_defaults() {
        let mut s = Selection::new();
        s.handle_key(KeyCode::Char('a'));
        assert_eq!(s.selected_ids().len(), CATEGORIES.len());
        s.handle_key(KeyCode::Char('d'));
        assert_eq!(s.selected_ids(), default_ids());
    }

    #[test]
    fn cursor_stays_in_bounds() {
        let mut s = Selection::new();
        s.handle_key(KeyCode::Up); // already at top
        assert_eq!(s.cursor, 0);
        for _ in 0..CATEGORIES.len() + 5 {
            s.handle_key(KeyCode::Down);
        }
        assert_eq!(s.cursor, CATEGORIES.len() - 1);
    }
}
