#![warn(clippy::all, rust_2018_idioms)]

mod app;
pub mod background_worker;
pub use app::DBV;

// TODO 1: Handle bug that when `Should round new points` is checked, points colors submenu is unreachable
// TODO 1: Color picker doesn't appear to be working even when it can be reached
