//! Reusable UI Components
//!
//! This module contains reusable UI components that can be used across
//! different screens in the TUI application.

// Layout components - implemented in Task 3.1
pub mod header;
pub mod modals;
pub mod navigation;
pub mod status_bar;

// Re-export components for easy access
pub use header::*;
pub use modals::*;
pub use navigation::*;
pub use status_bar::*;

// Data display components - implemented in Task 3.2
pub mod tables;

// Input components - implemented in Task 3.3
pub mod forms;

pub use forms::*;
pub use tables::*;
