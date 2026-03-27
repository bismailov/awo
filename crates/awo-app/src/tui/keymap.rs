use ratatui::prelude::Line;

pub(crate) fn help_lines() -> Vec<Line<'static>> {
    vec![
        Line::from(""),
        Line::from("  s       Acquire slot / Start selected task card"),
        Line::from("  /       Filter panels (case-insensitive on IDs/names)"),
        Line::from("  Enter   Start session / View log / Submit confirm"),
        Line::from("  x       Cancel session"),
        Line::from("  X       Release slot / Release selected task-card slot"),
        Line::from("  K       Delete released task-card slot"),
        Line::from("  a       Add repository by path"),
        Line::from("  c       Context doctor / Create team (Teams tab)"),
        Line::from("  m       Add member (Team Dashboard)"),
        Line::from("  L       Promote selected member to current lead (Team Dashboard)"),
        Line::from("  u       Update selected member (Team Dashboard)"),
        Line::from("  d       Skills doctor / Remove selected member (Team Dashboard)"),
        Line::from("  n       Add task card (Team Dashboard)"),
        Line::from("  D       Delegate selected task card (Team Dashboard)"),
        Line::from("  A       Accept selected review-ready task card"),
        Line::from("  W       Send selected task card back for rework"),
        Line::from("  o       Open selected task-card log"),
        Line::from("  t       Start next task card"),
        Line::from("  R       Generate team report"),
        Line::from("  r       Refresh review / Refresh log"),
        Line::from("  T       Team Dashboard"),
        Line::from("  Tab     Next panel / Next dashboard pane"),
        Line::from("  Esc     Close panel / Cancel input"),
        Line::from("  q       Quit"),
        Line::from("  ?       This help"),
    ]
}
