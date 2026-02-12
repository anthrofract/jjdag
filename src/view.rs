use crate::model::Model;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, Paragraph},
};

pub const SELECTION_COLOR: Color = Color::Rgb(40, 42, 54);
pub const SAVED_SELECTION_COLOR: Color = Color::Rgb(33, 35, 45);

pub fn view(model: &mut Model, frame: &mut Frame) {
    let header = render_header(model);
    let log_list = render_log_list(model);
    let layout = render_layout(model, frame.area());
    frame.render_widget(header, layout[0]);
    frame.render_stateful_widget(log_list, layout[1], &mut model.log_list_state);
    model.log_list_layout = layout[1];
    if let Some(info_list) = render_info_list(model) {
        frame.render_widget(info_list, layout[2]);
    }
}

fn render_layout(model: &Model, area: Rect) -> std::rc::Rc<[Rect]> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(0),
            if let Some(info_list) = &model.info_list {
                Constraint::Length(info_list.lines.len() as u16 + 2)
            } else {
                Constraint::Length(0)
            },
        ])
        .split(area)
}

fn render_header(model: &Model) -> Paragraph<'_> {
    let mut header_spans = vec![
        Span::styled("repository: ", Style::default().fg(Color::Blue)),
        Span::styled(&model.display_repository, Style::default().fg(Color::Green)),
        Span::raw("  "),
        Span::styled("revset: ", Style::default().fg(Color::Blue)),
        Span::styled(&model.revset, Style::default().fg(Color::Green)),
    ];
    if model.global_args.ignore_immutable {
        header_spans.push(Span::styled(
            "  --ignore-immutable",
            Style::default().fg(Color::LightRed),
        ));
    }
    Paragraph::new(Line::from(header_spans))
}

fn render_log_list(model: &Model) -> List<'static> {
    let mut log_items = model.log_list.clone();
    apply_saved_selection_highlights(model, &mut log_items);
    List::new(log_items)
        .highlight_style(Style::new().bold().bg(SELECTION_COLOR))
        .scroll_padding(model.log_list_scroll_padding)
}

fn apply_saved_selection_highlights(model: &Model, log_items: &mut [ratatui::text::Text<'static>]) {
    let (saved_commit_idx, saved_file_diff_idx) = model.get_saved_selection_flat_log_idxs();

    if let Some(idx) = saved_commit_idx
        && let Some(item) = log_items.get_mut(idx)
    {
        apply_saved_selection_highlight(item);
    }

    if let Some(idx) = saved_file_diff_idx
        && let Some(item) = log_items.get_mut(idx)
    {
        apply_saved_selection_highlight(item);
    }
}

fn apply_saved_selection_highlight(text: &mut ratatui::text::Text<'static>) {
    text.style = text.style.bg(SAVED_SELECTION_COLOR);
    for line in &mut text.lines {
        for span in &mut line.spans {
            span.style = span.style.bg(SAVED_SELECTION_COLOR);
        }
    }
}

fn render_info_list(model: &Model) -> Option<List<'static>> {
    let info_list = model.info_list.as_ref()?;
    Some(
        List::new(info_list.clone()).block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::Blue)),
        ),
    )
}
