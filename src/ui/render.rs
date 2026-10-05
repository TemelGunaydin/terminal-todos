use super::{App, Filter, Focus, InputKind, Mode};
use crate::{
    model::{age, safe},
    project,
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, List, ListItem, Padding, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Wrap,
    },
};

#[derive(Clone, Copy)]
struct Theme {
    bg: Color,
    panel: Color,
    text: Color,
    muted: Color,
    border: Color,
    accent: Color,
    pending: Color,
    danger: Color,
    info: Color,
    selected: Color,
}
impl Theme {
    fn new(color: bool) -> Self {
        let rgb = |r, g, b| {
            if color {
                Color::Rgb(r, g, b)
            } else {
                Color::Reset
            }
        };
        Self {
            bg: rgb(17, 16, 32),
            panel: rgb(26, 23, 48),
            text: rgb(241, 237, 255),
            muted: rgb(159, 149, 183),
            border: rgb(62, 52, 91),
            accent: rgb(167, 139, 250),
            pending: rgb(240, 179, 138),
            danger: rgb(244, 130, 154),
            info: rgb(129, 140, 248),
            selected: rgb(48, 39, 79),
        }
    }
    fn block(self, title: &'static str, focused: bool) -> Block<'static> {
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(if focused { self.accent } else { self.border }))
            .style(Style::default().fg(self.text).bg(self.panel))
            .title(Line::styled(
                title,
                Style::default()
                    .fg(if focused { self.accent } else { self.muted })
                    .bold(),
            ))
    }
}

fn project_badge(name: &str, theme: Theme, color: bool) -> Span<'static> {
    let (r, g, b) = project::rgb(name);
    Span::styled(
        format!(" {} ", safe(name)),
        Style::default()
            .fg(if color {
                Color::Rgb(r, g, b)
            } else {
                Color::Reset
            })
            .bg(if color { theme.selected } else { Color::Reset })
            .bold(),
    )
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let theme = Theme::new(app.color);
    let area = frame.area();
    frame.render_widget(
        Block::new().style(Style::default().fg(theme.text).bg(theme.bg)),
        area,
    );
    if area.width < 60 || area.height < 20 {
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled(
                    " >_ TERMINAL TODOS",
                    Style::default().fg(theme.accent).bold(),
                ),
                Line::from(" Resize the terminal to at least 60 × 20."),
                Line::from(" q quit · Ctrl+C exit"),
            ]),
            area,
        );
        return;
    }
    let compact = area.height < 24;
    let [header, metrics, filters, body, footer] = Layout::vertical([
        Constraint::Length(if compact { 2 } else { 3 }),
        Constraint::Length(3),
        Constraint::Length(2),
        Constraint::Min(5),
        Constraint::Length(3),
    ])
    .margin(1)
    .areas(area);
    header_view(frame, app, header, theme);
    metrics_view(frame, app, metrics, theme);
    filters_view(frame, app, filters, theme);
    let [tasks, details] = if area.width >= 100 {
        Layout::horizontal([Constraint::Percentage(56), Constraint::Percentage(44)])
            .spacing(1)
            .areas(body)
    } else {
        Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)]).areas(body)
    };
    tasks_view(frame, app, tasks, theme);
    details_view(frame, app, details, theme);
    footer_view(frame, app, footer, theme);
    modal_view(frame, app, area, theme);
}

fn header_view(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let subtitle = if let Some(project) = &app.project {
        Line::from(vec![
            Span::styled(" Project ", Style::default().fg(theme.muted)),
            project_badge(project, theme, app.color),
        ])
    } else {
        Line::styled(
            " Your tasks. Nothing left behind.",
            Style::default().fg(theme.muted),
        )
    };
    let [brand, mode] =
        Layout::horizontal([Constraint::Min(30), Constraint::Length(18)]).areas(area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(" >_ ", Style::default().fg(theme.accent).bold()),
                Span::styled("TERMINAL TODOS", Style::default().fg(theme.text).bold()),
            ]),
            subtitle,
        ]),
        brand,
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled("◉  LOCAL FIRST", Style::default().fg(theme.accent).bold()),
            Line::styled("Saved locally", Style::default().fg(theme.muted)),
        ])
        .right_aligned(),
        mode,
    );
}

fn metrics_view(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let done = app.db.tasks.iter().filter(|task| task.completed).count();
    let total = app.db.tasks.len();
    let areas = Layout::horizontal([Constraint::Ratio(1, 3); 3])
        .spacing(1)
        .split(area);
    for (area, (label, count, color)) in areas.iter().zip([
        (" OPEN ", total - done, theme.pending),
        (" COMPLETED ", done, theme.accent),
        (" TOTAL ", total, theme.info),
    ]) {
        frame.render_widget(
            Paragraph::new(format!(" {count:02}"))
                .style(Style::default().fg(color).bold())
                .block(theme.block(label, false)),
            *area,
        );
    }
}

fn filters_view(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    let mut spans = vec![Span::raw(" ")];
    for (number, filter) in [(1, Filter::Open), (2, Filter::Done), (3, Filter::All)] {
        let style = if app.filter == filter {
            Style::default().fg(theme.accent).bold().underlined()
        } else {
            Style::default().fg(theme.muted)
        };
        spans.push(Span::styled(format!("{number} {}", filter.label()), style));
        spans.push(Span::raw("   "));
    }
    if !app.query.is_empty() {
        spans.push(Span::styled(
            format!("/ {}", safe(&app.query)),
            Style::default().fg(theme.info),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn tasks_view(frame: &mut Frame, app: &mut App, area: Rect, theme: Theme) {
    let count = app.visible().len();
    let position = app.list.selected().map_or(0, |i| i + 1);
    let block = theme
        .block(" TASKS ", app.focus == Focus::Tasks)
        .title(Line::from(format!(" {position} / {count} ")).right_aligned());
    let inner = block.inner(area);
    app.page_size = (inner.height as usize / 2).max(1);
    let visible = app.visible();
    if visible.is_empty() {
        let message = if app.db.tasks.is_empty() {
            "\n  A clear space for your next task.\n\n  Press a to add your first task."
        } else if !app.query.is_empty() {
            "\n  No matching tasks.\n\n  Press Esc to clear your search."
        } else if app.filter == Filter::Open {
            "\n  All clear. Nothing left behind.\n\n  Press a to add a task, or 2 to see completed tasks."
        } else {
            "\n  No completed tasks yet.\n\n  Press 1 to return to open tasks."
        };
        frame.render_widget(
            Paragraph::new(message)
                .wrap(Wrap { trim: false })
                .style(Style::default().fg(theme.muted))
                .block(block),
            area,
        );
        return;
    }
    let items: Vec<_> = visible
        .iter()
        .map(|task| {
            let color = if task.completed {
                theme.accent
            } else {
                theme.pending
            };
            let text = if task.completed {
                theme.muted
            } else {
                theme.text
            };
            let mut metadata = vec![Span::styled(
                format!("     #{} ", task.id),
                Style::default().fg(theme.muted),
            )];
            if let Some(project) = &task.project {
                metadata.push(project_badge(project, theme, app.color));
            }
            metadata.push(Span::styled(
                format!(
                    " · {} · {}",
                    if task.completed { "Completed" } else { "Open" },
                    age(task.created_at)
                ),
                Style::default().fg(theme.muted),
            ));
            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(
                        if task.completed { " [x] " } else { " [ ] " },
                        Style::default().fg(color),
                    ),
                    Span::styled(safe(&task.title), Style::default().fg(text).bold()),
                ]),
                Line::from(metadata),
            ])
        })
        .collect();
    let highlight = if app.color {
        Style::default().bg(theme.selected)
    } else {
        Style::default().add_modifier(Modifier::REVERSED)
    };
    frame.render_stateful_widget(
        List::new(items)
            .block(block)
            .highlight_symbol("▎")
            .highlight_style(highlight),
        area,
        &mut app.list,
    );
    if count > app.page_size && area.height > 2 {
        let mut scrollbar = ScrollbarState::new(count * 2)
            .position(app.list.offset() * 2)
            .viewport_content_length(inner.height as usize);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .thumb_style(Style::default().fg(theme.accent))
                .track_style(Style::default().fg(theme.border)),
            area.inner(ratatui::layout::Margin {
                horizontal: 0,
                vertical: 1,
            }),
            &mut scrollbar,
        );
    }
}

fn details_view(frame: &mut Frame, app: &mut App, area: Rect, theme: Theme) {
    let block = theme
        .block(" DETAILS ", app.focus == Focus::Details)
        .padding(Padding::horizontal(1));
    let inner = block.inner(area);
    let lines = if let Some(task) = app.selected() {
        vec![
            Line::styled(safe(&task.title), Style::default().fg(theme.text).bold()),
            Line::from(""),
            Line::styled(
                if task.completed {
                    "✓ Completed"
                } else {
                    "○ Open"
                },
                Style::default()
                    .fg(if task.completed {
                        theme.accent
                    } else {
                        theme.pending
                    })
                    .bold(),
            ),
            Line::from(""),
            Line::styled("PROJECT", Style::default().fg(theme.muted)),
            if let Some(project) = &task.project {
                Line::from(project_badge(project, theme, app.color))
            } else {
                Line::styled("Unassigned", Style::default().fg(theme.muted))
            },
            Line::from(""),
            Line::styled("TASK ID", Style::default().fg(theme.muted)),
            Line::from(format!("#{} · IDs never change", task.id)),
            Line::from(""),
            Line::styled("CREATED", Style::default().fg(theme.muted)),
            Line::from(age(task.created_at)),
            Line::from(""),
            Line::styled("QUICK ACTIONS", Style::default().fg(theme.muted)),
            Line::from("Space  complete / reopen"),
            Line::from("e      edit task"),
            Line::from("d      delete with confirmation"),
        ]
    } else {
        vec![
            Line::from("Select a task to see its details."),
            Line::from(""),
            Line::styled(
                "Everything stays on your device.",
                Style::default().fg(theme.muted),
            ),
        ]
    };
    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    app.detail_max = paragraph
        .line_count(inner.width)
        .saturating_sub(inner.height as usize)
        .min(u16::MAX as usize) as u16;
    app.detail_scroll = app.detail_scroll.min(app.detail_max);
    app.detail_page = inner.height.max(1);
    let block = if app.detail_max > 0 {
        block.title(Line::from(" Tab · ↑↓ scroll ").right_aligned())
    } else {
        block
    };
    frame.render_widget(paragraph.scroll((app.detail_scroll, 0)).block(block), area);
}

fn footer_view(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(
                format!(" {}", safe(&app.message)),
                Style::default().fg(if app.error {
                    theme.danger
                } else {
                    theme.accent
                }),
            ),
            Line::styled(
                " ↑↓ / j k move · a add · e edit · Space complete · d delete",
                Style::default().fg(theme.muted),
            ),
            Line::styled(
                " 1/2/3 filter · / search · Tab panel · r refresh · q quit",
                Style::default().fg(theme.muted),
            ),
        ]),
        area,
    );
}

fn modal_view(frame: &mut Frame, app: &App, area: Rect, theme: Theme) {
    if matches!(app.mode, Mode::Normal) {
        return;
    }
    let width = area.width.saturating_sub(8).min(76);
    let height = 9;
    let modal = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, modal);
    match &app.mode {
        Mode::Input { kind, editor, .. } => {
            let (title, help) = match kind {
                InputKind::Add => (" ADD TASK ", "What would you like to get done?"),
                InputKind::Edit(_) => (" EDIT TASK ", "Update the task text."),
                InputKind::Search => (" SEARCH ", "Filter task titles as you type."),
            };
            let block = theme.block(title, true).padding(Padding::horizontal(1));
            let inner = block.inner(modal);
            frame.render_widget(block, modal);
            frame.render_widget(
                Paragraph::new(help).style(Style::default().fg(theme.muted)),
                Rect::new(inner.x, inner.y, inner.width, 1),
            );
            if matches!(kind, InputKind::Add) {
                let project = if let Some(project) = &app.project {
                    Line::from(vec![
                        Span::styled("Project: ", Style::default().fg(theme.muted)),
                        project_badge(project, theme, app.color),
                    ])
                } else {
                    Line::styled("Project: Unassigned", Style::default().fg(theme.muted))
                };
                frame.render_widget(
                    Paragraph::new(project),
                    Rect::new(inner.x, inner.y + 1, inner.width, 1),
                );
            }
            let input = Rect::new(inner.x, inner.y + 2, inner.width, 1);
            let (text, cursor) = editor.view(input.width);
            frame.render_widget(
                Paragraph::new(text).style(Style::default().fg(theme.text).bg(theme.selected)),
                input,
            );
            frame.set_cursor_position((input.x + cursor, input.y));
            let hint = if matches!(kind, InputKind::Search) {
                "Enter apply · Esc cancel · Ctrl+U clear"
            } else {
                "Enter save · Esc cancel · Ctrl+U clear"
            };
            frame.render_widget(
                Paragraph::new(hint).style(Style::default().fg(theme.muted)),
                Rect::new(inner.x, inner.y + 4, inner.width, 1),
            );
            if app.error {
                frame.render_widget(
                    Paragraph::new(safe(&app.message)).style(Style::default().fg(theme.danger)),
                    Rect::new(inner.x, inner.y + 6, inner.width, 1),
                );
            }
        }
        Mode::ConfirmDelete(task) => {
            let mut lines = vec![
                Line::styled(
                    format!("Delete task #{}?", task.id),
                    Style::default().fg(theme.danger).bold(),
                ),
                Line::from(safe(&task.title)),
                Line::from(""),
                Line::styled(
                    "This permanently removes the task.",
                    Style::default().fg(theme.muted),
                ),
                Line::from(""),
                Line::styled(
                    "Enter / y delete · Esc / n cancel",
                    Style::default().fg(theme.pending),
                ),
            ];
            if app.error {
                lines.push(Line::styled(
                    safe(&app.message),
                    Style::default().fg(theme.danger),
                ));
            }
            frame.render_widget(
                Paragraph::new(lines).block(
                    theme
                        .block(" DELETE TASK ", true)
                        .padding(Padding::horizontal(1)),
                ),
                modal,
            );
        }
        Mode::Normal => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Database;
    use anyhow::Result;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn layouts_and_modals_render_at_supported_and_tiny_sizes() -> Result<()> {
        for (width, height) in [(120, 32), (100, 24), (80, 24), (60, 20), (30, 10), (1, 1)] {
            let mut db = Database::default();
            db.add(&"Türkçe görev 🦀 ".repeat(30))?;
            db.add("Completed")?;
            db.set_completed(2, true)?;
            let mut app = App::new(db, true, None);
            let mut terminal = Terminal::new(TestBackend::new(width, height))?;
            terminal.draw(|frame| draw(frame, &mut app))?;
            app.key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char('a'),
                crossterm::event::KeyModifiers::NONE,
            ));
            terminal.draw(|frame| draw(frame, &mut app))?;
            app.mode = Mode::ConfirmDelete(app.db.tasks[0].clone());
            terminal.draw(|frame| draw(frame, &mut app))?;
        }
        Ok(())
    }

    #[test]
    fn dashboard_uses_its_own_violet_indigo_palette() -> Result<()> {
        let mut terminal = Terminal::new(TestBackend::new(120, 32))?;
        let mut app = App::new(Database::default(), true, None);
        terminal.draw(|frame| draw(frame, &mut app))?;
        let cells = terminal.backend().buffer().content();
        assert!(
            cells
                .iter()
                .any(|cell| cell.fg == Color::Rgb(167, 139, 250))
        );
        assert!(cells.iter().any(|cell| cell.bg == Color::Rgb(17, 16, 32)));
        assert!(cells.iter().all(|cell| cell.fg != Color::Rgb(82, 219, 175)));
        Ok(())
    }

    #[test]
    fn project_badges_are_visible_colored_and_safe_at_all_sizes() -> Result<()> {
        let name = "Bookfun";
        for color in [true, false] {
            for (width, height) in [(120, 32), (80, 24), (60, 20)] {
                let mut db = Database::default();
                db.add_for_project("Project task", Some(name))?;
                db.add_for_project("Long project task", Some(&"Türkçe 🦀 ".repeat(40)))?;
                let mut app = App::new(db, color, Some(name.into()));
                let mut terminal = Terminal::new(TestBackend::new(width, height))?;
                terminal.draw(|frame| draw(frame, &mut app))?;
                let cells = terminal.backend().buffer().content();
                let text: String = cells.iter().map(|cell| cell.symbol()).collect();
                assert!(text.contains(name));
                if color {
                    let (r, g, b) = project::rgb(name);
                    assert!(cells.iter().any(|cell| cell.fg == Color::Rgb(r, g, b)));
                } else {
                    assert!(
                        cells
                            .iter()
                            .all(|cell| cell.fg == Color::Reset && cell.bg == Color::Reset)
                    );
                }
                app.key(crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Char('a'),
                    crossterm::event::KeyModifiers::NONE,
                ));
                terminal.draw(|frame| draw(frame, &mut app))?;
                let text: String = terminal
                    .backend()
                    .buffer()
                    .content()
                    .iter()
                    .map(|cell| cell.symbol())
                    .collect();
                assert!(text.contains("Project:"));
            }
        }
        Ok(())
    }

    #[test]
    fn dashboard_shows_brand_metrics_and_empty_state_without_color() -> Result<()> {
        let mut terminal = Terminal::new(TestBackend::new(120, 32))?;
        let mut app = App::new(Database::default(), false, None);
        terminal.draw(|frame| draw(frame, &mut app))?;
        let buffer = terminal.backend().buffer();
        let text: String = buffer.content().iter().map(|cell| cell.symbol()).collect();
        for label in [
            "TERMINAL TODOS",
            "OPEN",
            "COMPLETED",
            "TOTAL",
            "Press a to add your first task.",
        ] {
            assert!(text.contains(label), "Missing {label}");
        }
        assert!(
            buffer
                .content()
                .iter()
                .all(|cell| cell.fg == Color::Reset && cell.bg == Color::Reset)
        );
        Ok(())
    }
}
