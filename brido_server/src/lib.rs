// Shared library crate for brido binaries.
//
// Both `brido-server` and `brido-overlay` depend on these modules.
// Server-specific modules (ai_server, stream_server, tray, tls, ui) remain
// private to the server binary's own `mod` declarations.

pub mod capture;
pub mod capture_gdi;
pub mod config;
pub mod encoder;
pub mod model_manager;
