use std::io::{self, Stdout};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use veiltype::clipboard::copy_to_clipboard;
use veiltype::editor::{EditorAction, EditorState};
use veiltype::fake::{FakeTyper, LanguageProfile};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Terminal;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Color as SynColor, Theme, ThemeSet};
use syntect::parsing::SyntaxSet;

fn main() -> Result<()> {
    run()
}

fn run() -> Result<()> {
    let mut terminal = setup_terminal()?;
    let mut app = App::new();

    let mut final_text = None;
    let mut cancelled = false;

    loop {
        terminal.draw(|f| app.render(f))?;

        if event::poll(Duration::from_millis(75))? {
            let ev = event::read()?;
            match ev {
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat {
                        continue;
                    }

                    let action = app.real.handle_key(key);
                    if action == EditorAction::Continue {
                        app.fake_typer.apply_to_decoy(key, &mut app.fake);
                    }

                    match action {
                        EditorAction::Continue => {}
                        EditorAction::Cancel => {
                            cancelled = true;
                            break;
                        }
                        EditorAction::SaveAndExit => {
                            final_text = Some(app.real.buffer().to_owned());
                            break;
                        }
                    }
                }
                Event::Paste(pasted) => {
                    app.real.insert_str(&pasted);
                    app.fake_typer.apply_paste_to_decoy(&pasted, &mut app.fake);
                }
                _ => {}
            }
        }
    }

    teardown_terminal(&mut terminal)?;

    if cancelled {
        println!("ct: cancelled (nothing copied)");
        return Ok(());
    }

    let typed = final_text.unwrap_or_default();
    let _ = copy_to_clipboard(&typed).context("copy failed")?;

    Ok(())
}

struct App {
    real: EditorState,
    fake: EditorState,
    fake_typer: FakeTyper,
    profile: LanguageProfile,
    syn: SyntaxHighlighter,
    scroll_line: usize,
    started_at: Instant,
}

impl App {
    fn new() -> Self {
        let fake_typer = FakeTyper::new();
        let profile = fake_typer.profile();
        Self {
            real: EditorState::new(),
            fake: EditorState::new(),
            fake_typer,
            profile,
            syn: SyntaxHighlighter::new(),
            scroll_line: 0,
            started_at: Instant::now(),
        }
    }

    fn render(&mut self, frame: &mut ratatui::Frame) {
        let root = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Min(2),
                Constraint::Length(1),
            ])
            .split(root);

        let topbar = Paragraph::new(Line::from(vec![
            Span::styled(
                "  EXPLORER  ",
                Style::default()
                    .fg(Color::Rgb(40, 45, 60))
                    .bg(Color::Rgb(224, 227, 235)),
            ),
            Span::styled(
                "  src  ",
                Style::default()
                    .fg(Color::Rgb(210, 216, 230))
                    .bg(Color::Rgb(54, 60, 76)),
            ),
            Span::styled(
                format!("  {}  ", self.profile.file_name),
                Style::default()
                    .fg(Color::Rgb(245, 248, 255))
                    .bg(Color::Rgb(67, 74, 92)),
            ),
            Span::styled(
                format!("  UTF-8  {}  ", self.profile.display_name),
                Style::default()
                    .fg(Color::Rgb(180, 188, 205))
                    .bg(Color::Rgb(38, 44, 57)),
            ),
        ]));
        frame.render_widget(topbar, chunks[0]);

        let title = Paragraph::new(format!(
            "Workspace: goatfood-tui    Branch: main    LSP: {}    {}",
            self.profile.lsp, self.profile.breadcrumb
        ))
            .style(Style::default().fg(Color::Rgb(210, 230, 255)))
            .block(Block::default().title(" ct editor ").borders(Borders::ALL));
        frame.render_widget(title, chunks[1]);

        let editor_area = chunks[2];
        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(24), Constraint::Min(20)])
            .split(editor_area);

        let sidebar = split[0];
        let sidebar_lines = vec![
            Line::from(Span::styled(
                "goatfood-tui",
                Style::default().fg(Color::Rgb(172, 188, 214)),
            )),
            Line::from("  src"),
            Line::from("    core"),
            Line::from(Span::styled(
                format!("    > {}", self.profile.file_name),
                Style::default()
                    .fg(Color::Rgb(220, 234, 255))
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  tests"),
            Line::from("  scripts"),
            Line::from(""),
            Line::from(Span::styled(
                "OUTLINE",
                Style::default().fg(Color::Rgb(130, 145, 170)),
            )),
            Line::from("  - bootstrap"),
            Line::from("  - hydrate"),
            Line::from("  - flush"),
        ];
        let sidebar_widget = Paragraph::new(sidebar_lines)
            .block(Block::default().title(" files ").borders(Borders::ALL))
            .style(Style::default().fg(Color::Rgb(158, 172, 196)));
        frame.render_widget(sidebar_widget, sidebar);

        let block = Block::default().title(" editor ").borders(Borders::ALL);
        let inner = block.inner(split[1]);
        frame.render_widget(block, split[1]);

        let visible_lines = inner.height.max(1) as usize;
        let (cursor_line, cursor_col) = self.fake.line_col();

        if cursor_line < self.scroll_line {
            self.scroll_line = cursor_line;
        }
        if cursor_line >= self.scroll_line + visible_lines {
            self.scroll_line = cursor_line.saturating_sub(visible_lines.saturating_sub(1));
        }

        let lines = self.render_lines(visible_lines, cursor_line);
        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);

        let cursor_x = inner.x + 6 + cursor_col as u16;
        let cursor_y = inner.y + cursor_line.saturating_sub(self.scroll_line) as u16;
        let blink_on = (self.started_at.elapsed().as_millis() / 550).is_multiple_of(2);
        if blink_on && cursor_y < inner.y + inner.height && cursor_x < inner.x + inner.width {
            frame.set_cursor_position((cursor_x, cursor_y));
        }

        let diagnostics = self.diagnostics_text();
        let status = format!(
            "NORMAL  Ln {}, Col {}   Spaces: 4   {}   hidden: {} chars   Ctrl+S Save   Ctrl+Q/Ctrl+Z Quit",
            cursor_line + 1,
            cursor_col + 1,
            diagnostics,
            self.real.buffer().chars().count()
        );
        let status_widget = Paragraph::new(status)
            .style(
                Style::default()
                    .fg(Color::Rgb(232, 237, 247))
                    .bg(Color::Rgb(37, 67, 106))
                    .add_modifier(Modifier::BOLD),
            );
        frame.render_widget(status_widget, chunks[3]);
    }

    fn render_lines(&self, visible_lines: usize, cursor_line: usize) -> Vec<Line<'static>> {
        let all: Vec<&str> = if self.fake.buffer().is_empty() {
            vec![""]
        } else {
            self.fake.buffer().split('\n').collect()
        };

        let mut rendered = Vec::with_capacity(visible_lines);
        let end = (self.scroll_line + visible_lines).min(all.len());
        for (row, line) in all[self.scroll_line..end].iter().enumerate() {
            let line_no = self.scroll_line + row + 1;
            let is_current = line_no == cursor_line + 1;
            let mut spans = vec![Span::styled(
                format!("{:>4}  ", line_no),
                Style::default()
                    .fg(Color::Rgb(105, 120, 140))
                    .bg(if is_current {
                        Color::Rgb(36, 44, 57)
                    } else {
                        Color::Reset
                    }),
            )];
            spans.extend(self.syn.highlight(line, self.profile.extension, is_current));
            rendered.push(Line::from(spans));
        }

        while rendered.len() < visible_lines {
            rendered.push(Line::from(Span::styled(
                "~",
                Style::default().fg(Color::Rgb(80, 96, 118)),
            )));
        }
        rendered
    }

    fn diagnostics_text(&self) -> &'static str {
        let size = self.real.buffer().chars().count();
        match (size / 19) % 5 {
            0 => "0 errors, 0 warnings",
            1 => "1 warning: dead_code",
            2 => "indexing workspace…",
            3 => "types checked",
            _ => "tests cached",
        }
    }
}

struct SyntaxHighlighter {
    ps: SyntaxSet,
    theme: Theme,
}

impl SyntaxHighlighter {
    fn new() -> Self {
        let ps = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();
        let theme = ts
            .themes
            .get("base16-ocean.dark")
            .cloned()
            .unwrap_or_default();
        Self { ps, theme }
    }

    fn highlight(&self, text: &str, ext: &str, is_current: bool) -> Vec<Span<'static>> {
        let syntax = self
            .ps
            .find_syntax_by_extension(ext)
            .unwrap_or_else(|| self.ps.find_syntax_plain_text());
        let mut h = HighlightLines::new(syntax, &self.theme);
        let ranges = h.highlight_line(text, &self.ps).unwrap_or_default();

        if ranges.is_empty() {
            return vec![Span::raw(" ")];
        }

        let line_bg = if is_current {
            Some(Color::Rgb(36, 44, 57))
        } else {
            None
        };

        ranges
            .into_iter()
            .map(|(style, slice)| {
                let c = style.foreground;
                let mut sty = Style::default().fg(syntect_to_ratatui(c));
                if let Some(bg) = line_bg {
                    sty = sty.bg(bg);
                }
                Span::styled(slice.to_owned(), sty)
            })
            .collect()
    }
}

fn syntect_to_ratatui(c: SynColor) -> Color {
    Color::Rgb(c.r, c.g, c.b)
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, Hide)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend).context("failed to create terminal")
}

fn teardown_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), Show, LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
