//! The GPUI front end for SeEx.
//!
//! This tree is the replacement for the egui UI that still lives in `src/main.rs`
//! on the `refactor/gpui-ui` branch. It is compiled both by the `seex-gpui` bin
//! (which pulls the non-UI modules in with `#[path]`) and, once the port lands,
//! by the main binary directly.

pub mod app;
pub mod components;
pub mod fonts;
pub mod locale;
pub mod pages;
pub mod theme;

pub use app::run;
