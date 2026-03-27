use crate::tui::forms::{ConfirmState, FormState};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

pub(crate) fn render_form_overlay(frame: &mut Frame, form: &FormState) {
    let area = centered_rect(70, form_height(form), frame.area());
    let mut lines = vec![Line::from("")];

    for (index, field) in form.fields.iter().enumerate() {
        let marker = if index == form.selected { ">" } else { " " };
        let suffix = if field.is_choice() {
            " [left/right]"
        } else {
            ""
        };
        lines.push(Line::from(format!(
            "{marker} {}: {}{}",
            field.label, field.value, suffix
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(format!(
        "Tab/Shift+Tab move  Enter {}  Esc cancel",
        form.submit_label
    )));

    if let Some(footer) = &form.footer {
        lines.push(Line::from(footer.clone()));
    }

    if let Some(error) = &form.error {
        lines.push(Line::from(""));
        lines.push(Line::styled(
            format!("Error: {error}"),
            Style::default().fg(Color::Red),
        ));
    }

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(form.title.clone()),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(Clear, area);
    frame.render_widget(widget, area);
}

pub(crate) fn render_confirm_overlay(frame: &mut Frame, confirm: &ConfirmState) {
    let area = centered_rect(60, 7, frame.area());
    let widget = Paragraph::new(confirm.message.clone())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(confirm.title.clone()),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(Clear, area);
    frame.render_widget(widget, area);
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let width = area
        .width
        .saturating_mul(percent_x)
        .saturating_div(100)
        .max(24);
    let height = height.min(area.height.saturating_sub(2)).max(5);
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

fn form_height(form: &FormState) -> u16 {
    let extra = usize::from(form.footer.is_some()) + usize::from(form.error.is_some()) + 4;
    (form.fields.len() + extra).min(u16::MAX as usize) as u16
}
