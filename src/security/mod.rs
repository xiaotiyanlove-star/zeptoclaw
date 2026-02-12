//! Security module for ZeptoClaw
//!
//! This module provides security utilities including path validation
//! and command filtering to prevent malicious tool execution.

pub mod path;
pub mod shell;

pub use path::{validate_path_in_workspace, SafePath};
pub use shell::ShellSecurityConfig;
