//! Pieces shared across both settings sections (`ui` and `providers`).
//!
//! The settings window mutates the document through a single, section-agnostic
//! [`super::save_settings`] command; that command emits one `settings-updated`
//! event whose payload is the [`super::SettingsUpdate`] it saved, with provider
//! keys redacted to the placeholder. Subscribers merge the pushed content
//! instead of refetching.
//!
//! On-demand facets (env-key presence, live model listing) ride their own
//! command read paths and never appear here.

use tauri::{AppHandle, Emitter};

use crate::settings::{SettingsCategory, SettingsUpdatedPayload};

use super::SettingsUpdate;

/// Emit `settings-updated` with the changed sections as the payload.
pub(super) fn emit_settings_updated(app: &AppHandle, update: SettingsUpdate) {
    let mut categories: Vec<SettingsCategory> = Vec::new();

    if !update.ui.is_none() {
        categories.push(SettingsCategory::Ui);
    }
    if !update.llm_providers.is_none() {
        categories.push(SettingsCategory::LlmProviders);
    }

    let _ = app.emit("settings-updated", SettingsUpdatedPayload { categories });
}
