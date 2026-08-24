mod actions;
mod controls;
mod cover;
mod discord;
mod karaoke;
mod progress;
#[cfg(target_os = "windows")]
pub(crate) use progress::TitlebarProgress;
mod viz;
#[cfg(target_os = "linux")]
mod volume;
mod window;
pub use window::{build_ui, UiEvent, UiOptions, UiResetReason};
