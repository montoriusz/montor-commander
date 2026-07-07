//! UI / appearance settings section.
//!
//! Owns only the `UiSettings` / `Theme` data model. Mutating the UI section
//! goes through the section-agnostic [`super::save_settings`] command; this
//! module emits no events and owns no commands.

use serde::{Deserialize, Serialize};

/// UI / appearance preferences.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UiSettings {
    pub theme: Theme,
    /// Terminal font size in points.
    pub font_size: u16,
    /// UI zoom multiplier as a percentage of the base scale.
    /// Range: `50..=200` (50% – 200%).
    pub ui_scale: u16,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            font_size: 13,
            ui_scale: 100,
        }
    }
}

/// Window colour scheme preference.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}
