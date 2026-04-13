use std::{io, time::Duration};

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Terminal,
};
use rovdex_core::{Context, EchoProvider, Engine, Task};

pub fn run(demo: bool) -> Result<()> {
    let engine = Engine::with_standard_tools(EchoProvider);
    let mut app = App::new(engine, demo);
    let mut session = TerminalSession::enter()?;

    run_app(session.terminal_mut(), &mut app)
}

pub fn preview(demo: bool) -> String {
    let status = if demo { "demo mode" } else { "ready" };
    [
        "+---------------------------------------------------------------+",
        "| Rovdex | Rust Coding Agent | provider: echo | status: ready  |",
        "+--------------------------+----------------------+-------------+",
        "| Navigator                | Transcript           | Inspector   |",
        "| - Chat                   | Rovdex session       | Session     |",
        "| - Tasks                  | initialized.         | - provider  |",
        "| - Tools                  | Ready for repository |   echo      |",
        "| - Diffs                  | tasks.               | - mode      |",
        "| - Logs                   |                      |   local     |",
        "|                          | user> inspect src/   | - tools: 4  |",
        "|                          | assistant> I can     |             |",
        "|                          | inspect repo and use | Quick Keys  |",
        "|                          | tools.               | - Enter     |",
        "|                          |                      | - Esc       |",
        "+--------------------------+----------------------+-------------+",
        "| Prompt                                                        |",
        "| inspect this repository                                       |",
        "+---------------------------------------------------------------+",
        "",
        "Welcome overlay:",
        "+---------------------------------------------------------------+",
        "| Welcome: Rovdex Dashboard                                     |",
        "| A Rust coding agent combining Codex-style execution with      |",
        "| Claude Code-style workflow.                                  |",
        "| Suggested prompts:                                            |",
        "| - inspect this repository                                     |",
        "| - explain the project structure                               |",
        "| - show current working directory                              |",
        "+---------------------------------------------------------------+",
        "",
        if demo {
            "Preview mode: demo transcript preloaded."
        } else {
            "Preview mode: empty live session layout."
        },
        status,
    ]
    .join("\n")
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
    show_welcome: bool,
}

impl App {
    fn new(engine: Engine<EchoProvider>, demo: bool) -> Self {
        let mut history = vec![
            String::from("Rovdex session initialized."),
            String::from("Ready for repository tasks."),
        ];

        if demo {
            history.extend([
                String::from("user> inspect src/ and explain the architecture"),
                String::from(
                    "assistant> I can inspect the repository, read files, and report structure.",
                ),
                String::from("tool[current_directory]> cwd: /workspace/rovdex"),
                String::from("tool[list_directory]> crates/\nREADME.md\nCargo.toml"),
            ]);
        }

        Self {
            engine,
            input: String::new(),
            history,
            status: if demo {
                String::from("demo mode")
            } else {
                String::from("ready")
            },
            should_quit: false,
            show_welcome: true,
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Esc => {
                if self.show_welcome {
                    self.show_welcome = false;
                } else {
                    self.should_quit = true;
                }
            }
            KeyCode::Enter => {
                self.show_welcome = false;
                self.submit()?;
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.show_welcome = false;
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

        self.history.push(format!("user> {prompt}"));

        match Context::from_current_dir().and_then(|context| {
            self.engine
                .run(context, Task::new("session", prompt.clone()))
        }) {
            Ok(result) => {
                self.history
                    .push(format!("assistant> {}", result.final_message));
                self.status = format!(
                    "provider: {} | iterations: {}",
                    self.engine.provider_name(),
                    result.iterations
                );
            }
            Err(error) => {
                self.history.push(format!("error> {error}"));
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
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(frame.area());

    draw_header(frame, layout[0], app);
    draw_body(frame, layout[1], app);
    draw_input(frame, layout[2], app);

    if app.show_welcome {
        draw_welcome_modal(frame);
    } else {
        let cursor_x = layout[2]
            .x
            .saturating_add(1 + app.input.chars().count() as u16);
        let cursor_y = layout[2].y.saturating_add(1);
        frame.set_cursor_position(Position::new(cursor_x, cursor_y));
    }
}

fn draw_header(frame: &mut ratatui::Frame<'_>, area: Rect, app: &App) {
    let title = Line::from(vec![
        Span::styled(
            "Rovdex",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  |  Rust Coding Agent"),
        Span::raw("  |  "),
        Span::styled(app.status.clone(), Style::default().fg(Color::Yellow)),
    ]);

    let header = Paragraph::new(title).block(Block::default().borders(Borders::ALL));
    frame.render_widget(header, area);
}

fn draw_body(frame: &mut ratatui::Frame<'_>, area: Rect, app: &App) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(28),
            Constraint::Min(40),
            Constraint::Length(28),
        ])
        .split(area);

    let navigation = List::new(vec![
        ListItem::new("Chat"),
        ListItem::new("Tasks"),
        ListItem::new("Tools"),
        ListItem::new("Diffs"),
        ListItem::new("Logs"),
    ])
    .block(Block::default().borders(Borders::ALL).title("Navigator"))
    .highlight_style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_widget(navigation, columns[0]);

    let transcript = app
        .history
        .iter()
        .map(|line| Line::from(line.clone()))
        .collect::<Vec<_>>();
    let transcript = Paragraph::new(transcript)
        .block(Block::default().borders(Borders::ALL).title("Transcript"))
        .wrap(Wrap { trim: false });
    frame.render_widget(transcript, columns[1]);

    let side_panel = Paragraph::new(vec![
        Line::from("Session"),
        Line::from("- provider: echo"),
        Line::from("- mode: local"),
        Line::from("- tools: 4"),
        Line::from(""),
        Line::from("Quick Keys"),
        Line::from("- Enter: submit prompt"),
        Line::from("- Esc: close overlay / quit"),
        Line::from("- Ctrl-C: quit"),
        Line::from(""),
        Line::from("Planned"),
        Line::from("- provider switcher"),
        Line::from("- tool trace view"),
        Line::from("- patch preview"),
    ])
    .block(Block::default().borders(Borders::ALL).title("Inspector"))
    .wrap(Wrap { trim: false });
    frame.render_widget(side_panel, columns[2]);
}

fn draw_input(frame: &mut ratatui::Frame<'_>, area: Rect, app: &App) {
    let input = Paragraph::new(app.input.as_str())
        .block(Block::default().borders(Borders::ALL).title("Prompt"));
    frame.render_widget(input, area);
}

fn draw_welcome_modal(frame: &mut ratatui::Frame<'_>) {
    let area = centered_rect(60, 55, frame.area());
    frame.render_widget(Clear, area);

    let modal = Paragraph::new(vec![
        Line::from(Span::styled(
            "Rovdex Dashboard",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(
            "A Rust coding agent combining Codex-style execution with Claude Code-style workflow.",
        ),
        Line::from(""),
        Line::from("What you can do now:"),
        Line::from("- Type a prompt and press Enter"),
        Line::from("- Inspect the transcript panel"),
        Line::from("- Explore the dashboard layout"),
        Line::from(""),
        Line::from("Suggested prompts:"),
        Line::from("- inspect this repository"),
        Line::from("- explain the project structure"),
        Line::from("- show current working directory"),
        Line::from(""),
        Line::from("Press any key to start. Press Esc twice to quit."),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Welcome")
            .style(Style::default().bg(Color::Black)),
    )
    .wrap(Wrap { trim: false });

    frame.render_widget(modal, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
