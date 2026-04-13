use std::{io, time::Duration};

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Position},
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
    Terminal,
};
use rovdex_core::{Context, EchoProvider, Engine, Task};

pub fn run() -> Result<()> {
    let engine = Engine::with_standard_tools(EchoProvider);
    let mut app = App::new(engine);
    let mut session = TerminalSession::enter()?;

    run_app(session.terminal_mut(), &mut app)
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl TerminalSession {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;

        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;

        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }

    fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<io::Stdout>> {
        &mut self.terminal
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

struct App {
    engine: Engine<EchoProvider>,
    input: String,
    history: Vec<String>,
    status: String,
    should_quit: bool,
}

impl App {
    fn new(engine: Engine<EchoProvider>) -> Self {
        Self {
            engine,
            input: String::new(),
            history: vec![String::from(
                "Press Enter to run the prompt. Esc or Ctrl-C exits.",
            )],
            status: String::from("ready"),
            should_quit: false,
        }
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Esc => {
                self.should_quit = true;
            }
            KeyCode::Enter => {
                self.submit()?;
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.input.push(ch);
            }
            _ => {}
        }

        Ok(())
    }

    fn submit(&mut self) -> Result<()> {
        let prompt = self.input.trim().to_string();
        self.input.clear();

        if prompt.is_empty() {
            self.status = String::from("empty prompt");
            return Ok(());
        }

        self.history.push(format!("> {prompt}"));

        match Context::from_current_dir().and_then(|context| {
            self.engine
                .run(context, Task::new("session", prompt.clone()))
        }) {
            Ok(result) => {
                self.history.push(result.final_message);
                self.status = format!(
                    "{} | iterations: {}",
                    self.engine.provider_name(),
                    result.iterations
                );
            }
            Err(error) => {
                self.history.push(format!("! {error}"));
                self.status = String::from("error");
            }
        }

        Ok(())
    }
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|frame| draw(frame, app))?;

        if app.should_quit {
            return Ok(());
        }

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                app.handle_key(key)?;
            }
        }
    }
}

fn draw(frame: &mut ratatui::Frame<'_>, app: &App) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(frame.area());

    let messages = if app.history.is_empty() {
        vec![Line::from("No messages yet")]
    } else {
        app.history
            .iter()
            .map(|line| Line::from(line.clone()))
            .collect()
    };

    let messages = Paragraph::new(messages)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Rovdex | {}", app.status)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(messages, layout[0]);

    let input = Paragraph::new(app.input.as_str())
        .block(Block::default().borders(Borders::ALL).title("Prompt"));
    frame.render_widget(input, layout[1]);

    let cursor_x = layout[1]
        .x
        .saturating_add(1 + app.input.chars().count() as u16);
    let cursor_y = layout[1].y.saturating_add(1);
    frame.set_cursor_position(Position::new(cursor_x, cursor_y));
}
