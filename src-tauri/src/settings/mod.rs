//! User settings: UI preferences and LLM provider/model configuration.
//!
//! [`SettingsState`] is the BE-owned, typed settings document, registered as a
//! managed Tauri state — mirroring [`crate::chat::ChatSession`]. The frontend
//! never touches the backing file directly; it mutates settings through a
//! single, section-agnostic [`save_settings`] command carrying a
//! [`SettingsUpdate`] (1..n sections). `save_settings` validates the present
//! sections, persists atomically, then emits one `settings-updated` event
//! whose payload is that same `SettingsUpdate` — so subscribers merge the
//! **content** of the changed sections without refetching.
//!
//! [`Settings`] holds **everything** the app persists — whether the value is
//! backed by `settings.json` or the OS keychain. What differs is how each
//! section is *mapped* in each direction (store, read, send-to-FE,
//! receive-from-FE). Secrets (provider API keys) live on the in-memory model
//! but are written to the keychain and stripped from the file; the FE only ever
//! sees a placeholder. Anything genuinely computed (which env keys are set,
//! which models a provider exposes) is fetched on demand via dedicated commands
//! ([`llm_providers::get_providers`], [`llm_providers::all_model_names`]),
//! not carried on the settings document.
//!
//! ## Layout
//!
//! The module is split by settings section, with shared pieces factored out:
//!
//! - [`shared`] — the `settings-updated` emit helper (payload: [`SettingsUpdate`]).
//! - [`ui`] — appearance section (`UiSettings`, `Theme`).
//! - [`llm_providers`] — LLM providers section: data model, validation, the
//!   four OS-keychain mappers (read / store / redact / reconcile), and the
//!   providers-specific commands (`all_model_names`, `get_providers`,
//!   `new_provider_id`).
//!
//! This file owns the root [`Settings`] document, [`SettingsUpdate`], the
//! [`SettingsState`] holder (load / apply / persist), and the section-agnostic
//! commands [`get_settings`] and [`save_settings`].
//!
//! ## Storage
//!
//! - Location: `app_config_dir()/settings.json` (config, not `app_data_dir`,
//!   which is used for chat sessions).
//! - Format: JSON — nested provider/model arrays round-trip cleanly through
//!   `serde`, and the on-disk shape matches the IPC wire shape.
//! - Writes are atomic (temp file + `rename` in the same directory).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

pub mod llm_providers;
pub mod shared;
pub mod ui;

// Re-export the data model so consumers can use `settings::*` paths (e.g.
// `settings::Provider`). Tauri `#[command]` functions are *not* re-exported
// here: `generate_handler!` resolves their generated `__cmd__` helpers through
// the literal module path (`settings::providers::set_provider_api_key`), which
// a `pub use` cannot satisfy. `lib.rs` therefore references section paths
// directly.
#[allow(unused_imports)] // public surface; consumers opt in as needed
pub use llm_providers::{LlmProviderSettings, ModelEntry, Provider};
#[allow(unused_imports)] // public surface; consumers opt in as needed
pub use ui::{Theme, UiSettings};

// ---------------------------------------------------------------------------
// Top-level document
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum SettingsCategory {
    Ui,
    LlmProviders,
}

/// Payload of the `settings-updated` event: the sections that changed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsUpdatedPayload {
    pub categories: Vec<SettingsCategory>,
}

/// Root settings document. Holds everything the app persists; the providers
/// section carries API keys in memory but stores them in the OS keychain rather
/// than `settings.json` (see [`llm_providers`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub ui: UiSettings,
    pub llm_providers: LlmProviderSettings,
}

/// Partial settings document.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SettingsUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui: Option<UiSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_providers: Option<LlmProviderSettings>,
}

// ---------------------------------------------------------------------------
// SettingsState
// ---------------------------------------------------------------------------

/// Owned settings state, managed via `app.manage`.
///
/// Holds the [`Settings`] document behind a `std::sync::Mutex` (critical
/// sections are short — validate, serialize, write, done) plus the path of the
/// backing file. The in-memory document carries real provider API keys (loaded
/// from the keychain on [`SettingsState::load`]); they are stripped on
/// [`SettingsState::persist`] and redacted before leaving the backend.
pub struct SettingsState {
    path: PathBuf,
    inner: Mutex<Settings>,
}

impl SettingsState {
    /// Load settings from `app_config_dir()/settings.json`, or defaults if the
    /// file is missing. A corrupt file is renamed to `settings.json.bad` so the
    /// user can recover, and defaults are used in its place.
    pub fn load(config_dir: &Path) -> Result<Self, String> {
        let path = config_dir.join("settings.json");
        let mut settings = if path.exists() {
            match fs::read_to_string(&path).and_then(|s| {
                serde_json::from_str::<Settings>(&s)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            }) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(error = %e, path = %path.display(), "settings.json parse failed; backing up");
                    let _ = fs::rename(&path, path.with_extension("json.bad"));
                    Settings::default()
                }
            }
        } else {
            Settings::default()
        };

        // Reading mapper: hydrate the in-memory provider keys from the keychain
        // (the file carries none).
        llm_providers::load_provider_keys(&mut settings.llm_providers.providers);

        Ok(Self {
            path,
            inner: Mutex::new(settings),
        })
    }

    /// Clone of the in-memory settings document (still carrying real keys —
    /// callers that hand it to the FE must redact first, see [`get_settings`]).
    fn settings(&self) -> Settings {
        self.inner.lock().unwrap().clone()
    }

    fn settings_by_categories(&self, sections: Vec<SettingsCategory>) -> SettingsUpdate {
        let mut result = SettingsUpdate::default();
        let settings = self.inner.lock().unwrap();

        for cat in sections {
            match cat {
                SettingsCategory::Ui => {
                    result.ui = Some(settings.ui.clone());
                }
                SettingsCategory::LlmProviders => {
                    result.llm_providers = Some(settings.llm_providers.clone());
                }
            }
        }

        result
    }

    /// Apply a partial update in place under the lock, mutating only the
    /// present sections. Called by [`save_settings`] after validation.
    fn apply(&self, update: &SettingsUpdate) {
        let mut s = self.inner.lock().unwrap();
        if let Some(ui) = &update.ui {
            s.ui = ui.clone();
        }
        if let Some(providers) = &update.llm_providers {
            s.llm_providers = providers.clone();
        }
    }

    /// Store provider keys in the keychain, then serialize a key-free copy and
    /// atomically overwrite the backing file.
    fn persist(&self) -> Result<(), String> {
        let s = self.inner.lock().unwrap();

        // Storing mapper (keychain): write/clear each provider's key.
        llm_providers::store_provider_keys(&s.llm_providers.providers)?;

        // Storing mapper (file): strip keys from the copy we serialize so the
        // plaintext document never holds a secret.
        let mut file_copy = s.clone();
        llm_providers::strip_provider_keys(&mut file_copy.llm_providers.providers);
        let json = serde_json::to_string_pretty(&file_copy)
            .map_err(|e| format!("serialize settings: {e}"))?;

        let dir = self
            .path
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "settings path has no parent".to_string())?;
        fs::create_dir_all(&dir).map_err(|e| format!("create config dir: {e}"))?;

        let tmp = dir.join("settings.json.tmp");
        fs::write(&tmp, json).map_err(|e| format!("write settings.json.tmp: {e}"))?;
        fs::rename(&tmp, &self.path).map_err(|e| format!("rename settings.json: {e}"))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tauri commands (section-agnostic)
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_settings(state: State<'_, SettingsState>) -> Settings {
    let mut settings = state.settings();
    llm_providers::redact_provider_keys(&mut settings.llm_providers.providers);
    settings
}

#[tauri::command]
pub fn get_settings_by_categories(
    categories: Vec<SettingsCategory>,
    state: State<'_, SettingsState>,
) -> SettingsUpdate {
    let mut settings = state.settings_by_categories(categories);

    if let Some(llm_providers) = &mut settings.llm_providers {
        llm_providers::redact_provider_keys(&mut llm_providers.providers);
    }

    settings
}

/// Save 1..n sections atomically, then emit one `settings-updated` event.
///
/// Flow: reconcile incoming provider keys against the stored ones ("receive from
/// FE" mapper) → validate each present section (all-or-nothing across sections)
/// → `apply` (mutates only present sections) → `persist` once (keys to keychain,
/// file stripped) → emit once with keys redacted back to the placeholder.
#[tauri::command]
pub fn save_settings(
    mut update: SettingsUpdate,
    app: AppHandle,
    state: State<'_, SettingsState>,
) -> Result<(), String> {
    if let Some(providers) = &mut update.llm_providers {
        // Receiving mapper: a returning placeholder means "keep the stored key".
        let current = state.settings().llm_providers.providers;
        llm_providers::apply_incoming_provider_keys(&current, &mut providers.providers);
        llm_providers::validate_providers(&providers.providers)?;
    }
    state.apply(&update);
    state.persist()?;

    shared::emit_settings_updated(&app, update);
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use genai::adapter::AdapterKind;
    use llm_providers::Provider;
    use ui::Theme;

    fn p(id: &str, name: &str, kind: AdapterKind, base_url: Option<&str>) -> Provider {
        Provider {
            id: id.into(),
            name: name.into(),
            alias: String::new(),
            kind,
            base_url: base_url.map(Into::into),
            models: vec![],
            enabled: true,
            is_primary: false,
            api_key: None,
        }
    }

    #[test]
    fn settings_default_not_empty() {
        let s = Settings::default();
        assert_eq!(s.ui.theme, Theme::System);
        assert_eq!(s.ui.font_size, 13);
        assert_eq!(s.ui.ui_scale, 100);
        assert!(s.llm_providers.providers.is_empty());
    }

    #[test]
    fn settings_roundtrip() {
        let mut s = Settings::default();
        s.llm_providers.providers.push(p(
            "a",
            "OpenAI compat",
            AdapterKind::OpenAI,
            Some("https://example.com/v1/"),
        ));
        s.llm_providers.providers[0].models.push(ModelEntry {
            name: "gpt-4o".into(),
            max_tokens: Some(128_000),
            max_output_tokens: Some(16_384),
            is_custom: true,
        });
        let json = serde_json::to_string(&s).unwrap();
        // No derived field names ever appear in the persisted JSON.
        assert!(!json.contains("hasStoredKey"));
        assert!(!json.contains("knownModels"));
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.llm_providers.providers.len(), 1);
        assert_eq!(back.llm_providers.providers[0].models.len(), 1);
        assert_eq!(back.llm_providers.providers[0].kind, AdapterKind::OpenAI);
    }

    #[test]
    fn settings_roundtrip_preserves_provider_alias() {
        // `alias` is `#[serde(default, skip_serializing_if = "String::is_empty")]`,
        // so an empty alias doesn't appear in the JSON but round-trips back to
        // the default; a non-empty alias survives a serialize/deserialize cycle.
        let mut s = Settings::default();
        s.llm_providers
            .providers
            .push(p("a", "OpenAI", AdapterKind::OpenAI, None));
        s.llm_providers.providers[0].alias = "work-1".into();

        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"alias\":\"work-1\""));
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.llm_providers.providers[0].alias, "work-1");

        // An unset alias stays absent from the JSON and reads back as empty.
        let s_default = Settings::default();
        let json_default = serde_json::to_string(&s_default).unwrap();
        assert!(!json_default.contains("alias"));
        let back_default: Settings = serde_json::from_str(&json_default).unwrap();
        assert_eq!(back_default.llm_providers.providers.len(), 0);
    }

    fn write_temp_settings(contents: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, contents).unwrap();
        (dir, path)
    }

    #[test]
    fn load_returns_defaults_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let state = SettingsState::load(dir.path()).unwrap();
        assert_eq!(state.settings(), Settings::default());
    }

    #[test]
    fn load_recovers_from_corrupt_file() {
        let (_dir, path) = write_temp_settings("{ not valid json");
        let state = SettingsState::load(path.parent().unwrap()).unwrap();
        assert_eq!(state.settings(), Settings::default());
    }

    #[test]
    fn apply_ui_only_leaves_providers_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let state = SettingsState::load(dir.path()).unwrap();
        // Seed providers via apply.
        state.apply(&SettingsUpdate {
            ui: None,
            llm_providers: Some(LlmProviderSettings {
                providers: vec![p("a", "OpenAI", AdapterKind::OpenAI, None)],
            }),
        });
        // A ui-only update must not touch providers.
        state.apply(&SettingsUpdate {
            ui: Some(UiSettings {
                theme: Theme::Dark,
                font_size: 20,
                ui_scale: 100,
            }),
            llm_providers: None,
        });
        let s = state.settings();
        assert_eq!(s.ui.theme, Theme::Dark);
        assert_eq!(s.ui.font_size, 20);
        assert_eq!(s.ui.ui_scale, 100);
        assert_eq!(s.llm_providers.providers.len(), 1);
    }

    #[test]
    fn apply_providers_only_leaves_ui_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let state = SettingsState::load(dir.path()).unwrap();
        let original_ui = state.settings().ui;
        state.apply(&SettingsUpdate {
            ui: None,
            llm_providers: Some(LlmProviderSettings {
                providers: vec![p("a", "OpenAI", AdapterKind::OpenAI, None)],
            }),
        });
        let s = state.settings();
        assert_eq!(s.ui, original_ui);
        assert_eq!(s.llm_providers.providers.len(), 1);
    }

    #[test]
    fn apply_multi_section_updates_both() {
        let dir = tempfile::tempdir().unwrap();
        let state = SettingsState::load(dir.path()).unwrap();
        state.apply(&SettingsUpdate {
            ui: Some(UiSettings {
                theme: Theme::Light,
                font_size: 15,
                ui_scale: 100,
            }),
            llm_providers: Some(LlmProviderSettings {
                providers: vec![p("a", "OpenAI", AdapterKind::OpenAI, None)],
            }),
        });
        let s = state.settings();
        assert_eq!(s.ui.theme, Theme::Light);
        assert_eq!(s.ui.font_size, 15);
        assert_eq!(s.ui.ui_scale, 100);
        assert_eq!(s.llm_providers.providers.len(), 1);
    }

    #[test]
    fn persist_atomic_write_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let state = SettingsState::load(dir.path()).unwrap();
        state.apply(&SettingsUpdate {
            ui: Some(UiSettings {
                theme: Theme::Dark,
                font_size: 14,
                ui_scale: 100,
            }),
            llm_providers: Some(LlmProviderSettings {
                providers: vec![p("a", "OpenAI", AdapterKind::OpenAI, Some("https://x/v1/"))],
            }),
        });
        state.persist().unwrap();

        // Reload from disk and confirm both sections survived.
        let reloaded = SettingsState::load(dir.path()).unwrap();
        let s = reloaded.settings();
        assert_eq!(s.ui.theme, Theme::Dark);
        assert_eq!(s.llm_providers.providers[0].name, "OpenAI");
    }
}
