use std::io::{self, Stdout};

use crossterm::cursor::{Hide, Show};
use crossterm::event::{DisableBracketedPaste, EnableBracketedPaste};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::error::Result;

pub struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    active: bool,
}

impl TerminalSession {
    pub fn enter() -> Result<Self> {
        enable_raw_mode()?;
        execute!(
            io::stdout(),
            EnterAlternateScreen,
            EnableBracketedPaste,
            Hide
        )?;

        let backend = CrosstermBackend::new(io::stdout());
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;

        Ok(Self {
            terminal,
            active: true,
        })
    }

    pub fn draw<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut ratatui::Frame),
    {
        self.terminal.draw(f)?;
        Ok(())
    }

    pub fn suspend<T, F>(&mut self, f: F) -> Result<T>
    where
        F: FnOnce() -> Result<T>,
    {
        self.restore()?;
        let out = f();
        self.reenter()?;
        out
    }

    pub fn suspend_keep_screen<T, F>(&mut self, f: F) -> Result<T>
    where
        F: FnOnce() -> Result<T>,
    {
        if !self.active {
            return f();
        }

        self.terminal.show_cursor()?;
        execute!(io::stdout(), Show, DisableBracketedPaste)?;
        disable_raw_mode()?;

        let out = f();

        enable_raw_mode()?;
        execute!(io::stdout(), EnableBracketedPaste, Hide)?;
        out
    }

    fn reenter(&mut self) -> Result<()> {
        enable_raw_mode()?;
        execute!(
            io::stdout(),
            EnterAlternateScreen,
            EnableBracketedPaste,
            Hide
        )?;
        self.terminal.clear()?;
        self.active = true;
        Ok(())
    }

    pub fn restore(&mut self) -> Result<()> {
        if !self.active {
            return Ok(());
        }

        self.terminal.show_cursor()?;
        execute!(
            io::stdout(),
            Show,
            DisableBracketedPaste,
            LeaveAlternateScreen
        )?;
        disable_raw_mode()?;
        self.active = false;
        Ok(())
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}
