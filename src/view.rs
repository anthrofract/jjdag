use std::str::FromStr;

use crate::model::Model;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, Paragraph},
};

const SELECTION_COLOR: &str = "#282A36";

pub fn view(model: &mut Model, frame: &mut Frame) {
    // Render header
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
    let header = Paragraph::new(Line::from(header_spans));

    // Render log list
    let mut log_list = model.log_list.clone();
    if let Some(saved_log_index) = model.saved_log_index {
        let bg_color = Color::from_str(SELECTION_COLOR).unwrap();
        let mut text = log_list[saved_log_index].clone();
        text.style = text.style.bg(bg_color);
        for line in &mut text.lines {
            for span in &mut line.spans {
                span.style = span.style.bg(bg_color);
            }
        }
        log_list[saved_log_index] = text;
    }
    let log_list = List::new(log_list)
        .highlight_style(
            Style::new()
                .bold()
                .bg(Color::from_str(SELECTION_COLOR).unwrap()),
        )
        .scroll_padding(model.log_list_scroll_padding);

    // Render layout
    let layout = Layout::default()
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
        .split(frame.area());
    frame.render_widget(header, layout[0]);
    frame.render_stateful_widget(log_list, layout[1], &mut model.log_list_state);
    model.log_list_layout = layout[1];

    // Render info list
    if let Some(info_list) = &model.info_list {
        let info_list = List::new(info_list.clone()).block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::Blue)),
        );
        frame.render_widget(info_list, layout[2]);
    }
}
