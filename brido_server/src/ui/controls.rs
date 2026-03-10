/// Actions the user can trigger from the control buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlAction {
    Restart,
    Minimize,
    Shutdown,
}
