use ratatui::prelude::Line;

pub(crate) fn help_lines() -> Vec<Line<'static>> {
    vec![
        Line::from(""),
        Line::from("  s       Acquire slot / Start selected team task"),
        Line::from("  /       Filter panels (case-insensitive on IDs/names)"),
        Line::from("  Enter   Start session / View log / Submit confirm"),
        Line::from("  x       Cancel session"),
        Line::from("  X       Release slot"),
        Line::from("  a       Add repository by path"),
        Line::from("  c       Context doctor / Create team (Teams tab)"),
        Line::from("  m       Add member (Team Dashboard)"),
        Line::from("  u       Update selected member (Team Dashboard)"),
        Line::from("  d       Skills doctor / Remove selected member (Team Dashboard)"),
        Line::from("  n       Add task (Team Dashboard)"),
        Line::from("  D       Delegate selected task (Team Dashboard)"),
        Line::from("  t       Start next team task"),
        Line::from("  R       Generate team report"),
        Line::from("  r       Refresh review / Refresh log"),
        Line::from("  T       Team Dashboard"),
        Line::from("  Tab     Next panel / Next dashboard pane"),
        Line::from("  Esc     Close panel / Cancel input"),
        Line::from("  q       Quit"),
        Line::from("  ?       This help"),
    ]
}
