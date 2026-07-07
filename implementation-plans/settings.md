# Settings — Implementation Plan

## Goal

A settings solution with:

- A backend-owned settings document (validated, persisted, event-emitting).
- A settings window with a **vertical tab** category list on the left and the
  corresponding form on the right.
- A **UI** category and an **LLM Providers** category (providers → models
  nesting handled inside the providers form).
- A **single** `save_settings` command that accepts a partial update carrying
  **1..n sections** at once. On save the BE validates the present sections,
  persists atomically, then emits **one** `settings-updated` event whose payload
  is that same partial update — so the event carries the **content** of the
  changed sections (no refetch needed).
- A single authoritative **`Settings` document** that holds *everything* the app
  persists — values backed by the file **and** secrets backed by the OS keychain.
  What differs per section is the set of **mappers** that move each field in each
  direction (store / read / send-to-FE / receive-from-FE). There is **no**
  separate "derived settings" document.
- Genuinely computed facets (which env-var keys are set, which models a provider
  exposes) are fetched **on demand** via dedicated commands, not carried on the
  document.
- Individual parts of the app can subscribe to individual settings categories.
- API keys resolved from the environment by default; optionally overridden via
  the UI and stored in the OS keychain (never in the plaintext settings file).
  The key lives on the in-memory `Provider`; the FE only ever sees a common
  placeholder standing in for a stored key.

This mirrors existing patterns: BE-owned managed state (`ChatSession`) and typed
IPC via `tauri-typegen`. The one deliberate divergence from `chat-store.ts`:
the settings frontend store is built on **Zustand** (selector-based slice
subscriptions) rather than a hand-rolled `useSyncExternalStore` singleton, and
the `settings-updated` event pushes changed content directly into the store
instead of triggering a refetch.

### The `SettingsUpdate` shape (bidirectional)

A single partial-document type flows in **both** directions:

- **Save (FE → BE):** `save_settings(update: SettingsUpdate)` — the sections the
  user changed. Absent sections are left untouched.
- **Event (BE → FE):** the `settings-updated` payload **is** a `SettingsUpdate`
  containing exactly the sections that changed. Because `Settings` is now pure
  (no derived fields), the echo is **literally the saved content** — no
  hydration step. Subscribers merge the present sections into their store.

This unifies the wire shape and removes the need for a separate
`SettingsUpdatedPayload` / `SettingsCategory` tag — "which section changed" is
encoded by which fields of the `SettingsUpdate` are present. Secrets never travel
unprotected: provider keys on both the `save_settings` input and the
`settings-updated` echo are the **placeholder**, not the real value (see
[Secrets & the four mappers](#secrets-and-the-four-mappers)).

---

## 1. Storage

- **Format:** JSON (nested provider/model arrays round-trip cleanly through
  `serde`; matches the IPC wire shape). No `tauri-plugin-store`.
- **Location:** `app.path().app_config_dir()` → `settings.json`.
- **Writes:** atomic (write temp file in the same dir, then `rename`).
- **Secrets:** API keys are **not** written to `settings.json`. The key is a
  real field on the in-memory `Provider` (`api_key`), loaded from / stored to the
  OS keychain by the providers-section mappers and stripped from the file copy.
  The FE receives a common placeholder in its place (see
  [Secrets & the four mappers](#secrets-and-the-four-mappers)).

### Crates to add (`src-tauri/Cargo.toml`)

- `keyring = "3"` — OS keychain access for API keys.
- (`serde`, `serde_json`, `tokio`, `tauri` already present.)

---

## 2. Backend

### 2.1 Module `src-tauri/src/settings/`

The module is split by settings section (already landed):

- `mod.rs` — root `Settings` document, `SettingsUpdate`, `SettingsState`
  (load / persist / apply), and the section-agnostic commands (`get_settings`,
  `save_settings`). `load`/`persist`/`get_settings`/`save_settings` each call the
  relevant section mappers.
- `ui.rs` — `UiSettings`, `Theme`. This section has no secrets, so its mappers
  are the identity (nothing to strip/redact/reconcile).
- `llm_providers.rs` — providers data model, `validate_providers` (`pub(super)`
  so `save_settings` can call it), the keychain helpers, the **four section
  mappers** (read / store / redact / reconcile — see below), and the
  providers-specific commands (`all_model_names`, `get_providers`,
  `new_provider_id`).

  > **Implementation drift:** shipped as `get_providers() -> Vec<ProviderMeta>`
  > (`{ kind, hasEnvKey, supportsCustomModels }` per adapter kind), not the
  > `provider_env_keys(kinds) -> HashMap<AdapterKind, bool>` described below —
  > it now also carries whether a kind supports user-added custom models, and
  > always returns every kind rather than a requested subset.
- `shared.rs` — the `emit_settings_updated` helper (emits a `SettingsUpdate`;
  the caller redacts secrets first). `SettingsCategory` / `SettingsUpdatedPayload`
  are removed.

Register `mod settings;` in `lib.rs`, `app.manage(SettingsState::load(&config_dir))`
in `setup`, and add the commands to `invoke_handler!`. Note: Tauri's
`generate_handler!` resolves each command through its literal module path, so
section-owned commands are registered as e.g.
`settings::llm_providers::all_model_names` (a `pub use` re-export does **not**
satisfy the macro's generated `__cmd__` lookup).

#### Data model

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub ui: UiSettings,
    pub providers: ProvidersSettings,
}

/// Partial settings document. Carries 1..n sections; absent sections are
/// untouched on save and "unchanged" in an event. Used for BOTH the
/// `save_settings` command body and the `settings-updated` event payload.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SettingsUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui: Option<UiSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub providers: Option<ProvidersSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UiSettings {
    // Seed with something real so the section is non-empty, e.g.:
    pub theme: Theme,          // System | Light | Dark
    pub font_size: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProvidersSettings {
    pub providers: Vec<Provider>,
    /// id of the provider+model selected for generation (replaces const MODEL).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_model: Option<SelectedModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: String,                    // stable uuid; keychain entry key
    pub name: String,                  // display label
    /// genai's own flat adapter enum. `OpenAI` doubles for OpenAI-compatible
    /// custom endpoints (set `base_url`). No souffleur-side `ProviderKind`/
    /// `NativeVendor` mirror; the FE `Provider['kind']` union is emitted by
    /// `tauri-typegen` via the `typeMappings` → `AdapterKind` entry in
    /// `tauri.conf.json` (the genai serde variant names).
    pub kind: genai::adapter::AdapterKind,
    /// Optional custom endpoint override on top of `kind`. When absent, genai
    /// uses the adapter's native endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    pub models: Vec<ModelEntry>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub is_primary: bool,
    /// API key backed by the OS keychain — **never** written to settings.json.
    /// It is (de)serialized over IPC (so the placeholder can round-trip to/from
    /// the FE); the file write path strips it explicitly. `None`/empty = no key
    /// (genai falls back to its env-var lookup). See the four mappers below.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelEntry {
    pub name: String,                  // model id, e.g. "gpt-4o"
    /// PRIMARY model settings (besides name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
}
```

<a id="secrets-and-the-four-mappers"></a>

##### Secrets & the four mappers

There is **no** `DerivedSettings` document. `Settings` holds everything that is
persisted, including the provider `api_key`. What makes secrets special is not a
separate type but the set of **per-section mappers** the root `SettingsState`
calls at each boundary. The `ui` section's mappers are the identity; the
`llm_providers` section defines four real ones (all `pub(super)` fns in
`llm_providers.rs`, operating on `&mut [Provider]`):

| Direction | Mapper | What it does |
|---|---|---|
| **read** (keychain → memory) | `load_provider_keys` | After `settings.json` is parsed, fill each `Provider.api_key` from the keychain entry keyed by `id`. Called by `SettingsState::load`. |
| **store** (memory → keychain + file) | `store_provider_keys` + `strip_provider_keys` | On `persist`: write/clear each key in the keychain, then serialize a **clone with keys stripped** to the file. Called by `SettingsState::persist`. |
| **send to FE** | `redact_provider_keys` | Replace a stored key with the common `API_KEY_PLACEHOLDER` (empty stays `None`). Called by `get_settings` and before emitting `settings-updated`. |
| **receive from FE** | `apply_incoming_provider_keys` | Reconcile incoming keys against the live ones: placeholder (or omitted) = keep stored key; empty string = clear; anything else = new key. Called by `save_settings` before validate/apply. |

```rust
/// Shown to the FE in place of a stored key; a returning value equal to this
/// means "leave the keychain entry as-is".
pub(super) const API_KEY_PLACEHOLDER: &str = "••••••••••••";
```

Because `api_key` is a normal (IPC-serialized) field, the placeholder can
round-trip to and from the FE through the same `Settings`/`SettingsUpdate` shapes;
it is kept out of the file purely by the `store` mapper, and kept secret purely
by the `send-to-FE` mapper. This keeps a **single** `Settings` type authoritative
in every direction, with the special handling localized to the section that owns
the secret.

Genuinely computed facets are **not** on the document at all; they are fetched on
demand by dedicated commands (see [Commands](#commands)):

- `get_providers() -> Vec<ProviderMeta>` — every adapter kind with whether its
  default key env var is set (`hasEnvKey`) and whether it supports
  user-added custom models (`supportsCustomModels`). (Shipped name/shape;
  originally planned as `provider_env_keys(kinds) -> HashMap<AdapterKind, bool>`.)
- `all_model_names(kind, base_url, key?, id?) -> Vec<String>` — live model list
  for a provider; when `key` is the placeholder (or omitted) and `id` is given,
  the stored key for that provider is used.

`base_url` is optional for every kind — a custom endpoint override on top of
the adapter named by `kind`. The provider form no longer distinguishes
"native" vs "compatible" provider kinds; the dropdown lists every `AdapterKind`
variant (genai serde names, e.g. `OpenAI`, `Gemini`, `Anthropic`, `Groq`,
`Ollama`, `DeepSeek`, `Cohere`, `Xai`, …). `models` is a flat user-curated
list for all kinds.

> Note: `tauri-typegen` cannot introspect external opaque types like
> `genai::adapter::AdapterKind`, so `tauri.conf.json` carries a
> `typeMappings` entry mapping the short name `AdapterKind` to the inline TS
> literal union of the genai serde variants. Keep that mapping in sync with
> the variants compiled into the souffleur `genai` dependency (currently
> all variants except the feature-gated `BedrockSigv4`).

#### `SettingsState`

```rust
pub struct SettingsState {
    path: PathBuf,
    inner: Mutex<Settings>,   // std::sync::Mutex is fine (short critical sections)
}
```

- `load(config_dir)`: read+parse `settings.json`, or `Settings::default()` if
  missing/corrupt (log on parse error, back up the bad file), **then run the
  read mapper** (`load_provider_keys`) to hydrate keys from the keychain.
- `persist(&self)`: run the store mappers (`store_provider_keys` to the keychain,
  `strip_provider_keys` on the file clone), then serialize + atomic write.
- `apply(&self, update: &SettingsUpdate)`: mutate the present sections in place
  under the lock (`if let Some(ui) = &update.ui { s.ui = ui.clone() }`, etc.).
  Called by `save_settings` after key reconciliation + validation.
- `settings(&self) -> Settings`: clone the in-memory document (still carrying
  real keys). Callers that hand it to the FE redact first (see `get_settings`).

#### API keys (keychain)

- Entry: `keyring::Entry::new("montor-commander", &provider.id)`.
- `set_provider_key(id, key)` → `entry.set_password(&key)`;
  `clear_provider_key(id)` → `entry.delete_credential()` (a missing keychain or
  entry is a no-op, so saving key-less settings never fails on a host without a
  secret service).
- `lookup_key(id) -> Option<String>`: keychain first, else `None` (env-var
  fallback is handled by `genai` itself — see 2.3).
- These are wrapped by the four section mappers
  ([Secrets & the four mappers](#secrets-and-the-four-mappers)); no keychain
  access happens outside `llm_providers.rs`.

#### Commands

```rust
#[tauri::command] fn get_settings(state) -> Settings   // keys redacted to the placeholder

/// Single, multi-section save. `update` carries 1..n sections.
#[tauri::command] fn save_settings(update: SettingsUpdate, app, state) -> Result<(), String>

// Providers-specific, on-demand facets (no derived document):
#[tauri::command] fn get_providers() -> Vec<ProviderMeta>   // { kind, hasEnvKey, supportsCustomModels }
#[tauri::command] async fn all_model_names(kind: AdapterKind, base_url: Option<String>, key: Option<String>, id: Option<String>, state) -> Result<Vec<String>, String>
#[tauri::command] fn new_provider_id() -> String
```

`save_settings` flow:

1. **Reconcile keys** for the `llm_providers` section via
   `apply_incoming_provider_keys(&current, &mut incoming)` — a returning
   placeholder (or omitted key) keeps the stored key; empty clears; anything
   else is a new key.
2. **Validate** each present section (`validate_providers`). All-or-nothing
   across the sections in one `update`.
3. **Apply** via `state.apply(&update)` (mutates only present sections; the
   in-memory document now holds real keys).
4. **Persist** once (`state.persist()` → keys to keychain, file stripped).
5. **Redact** the `update`'s provider keys back to the placeholder, then **emit**
   once: `emit_settings_updated(&app, update)`.

`get_settings` clones the document and runs `redact_provider_keys` before
returning. `all_model_names` resolves the key from `id` when the caller passes
the placeholder, so the UI can list models for a provider whose key it only sees
as the placeholder. `update_ui_settings` / `update_providers` /
`get_derived_settings` / `set_provider_api_key` / `clear_provider_api_key` are
**removed**.

#### Events

**One** event:

- **`settings-updated`** — payload is a **`SettingsUpdatedPayload { categories:
  Vec<SettingsCategory> }`** tag, not the `SettingsUpdate` content itself.
  `save_settings` emits it once with the categories it saved; subscribers pull
  the new values with [`get_settings_by_categories`] (or [`get_settings`]).

  > **Implementation drift:** the plan originally called for the event payload
  > to *be* the changed `SettingsUpdate` content, so subscribers could merge it
  > without refetching ("presence of a field = that section changed", no
  > `SettingsCategory` tag). The shipped design instead reintroduces
  > `SettingsCategory` / `SettingsUpdatedPayload` (see [`shared.rs`]) purely as
  > a change-signal, and the frontend store refetches the affected categories
  > via `get_settings_by_categories` — see §3.1.

```rust
fn emit_settings_updated(app: &AppHandle, update: SettingsUpdate) {
    let mut categories: Vec<SettingsCategory> = Vec::new();
    if !update.ui.is_none() { categories.push(SettingsCategory::Ui); }
    if !update.llm_providers.is_none() { categories.push(SettingsCategory::LlmProviders); }
    let _ = app.emit("settings-updated", SettingsUpdatedPayload { categories });
}
```

There is no derived/keychain event: because keys travel *inside* the settings
document (as the placeholder), a normal `settings-updated` already tells other
windows a provider's key state may have changed, and `get_settings`/
`get_settings_by_categories` return the current placeholder/empty state.
On-demand facets (`get_providers`, `all_model_names`) are pulled when the form
needs them.

> Cross-window note: `settings-updated` is a broadcast, so a providers change in
> one window reaches others. A key set/clear *does* go through `save_settings`
> now (the key rides the providers section), so it broadcasts like any other
> providers change.

### 2.2 Validation (BE)

- Provider `id` unique and non-empty; `name` non-empty.
- `base_url`, when present, is non-empty (a `None` means "use the adapter's
  native endpoint" — optional for every kind).
- Model `name` non-empty and unique within a provider.
- `max_output_tokens <= max_tokens` when both present.
- Return `Err(String)` with a user-facing message on failure (surfaced as a
  toast in the UI). Frontend Yup mirrors these for inline feedback.

### 2.3 Wire settings into generation (`chat/generation.rs`)

- Remove the hardcoded `const MODEL`; take the selected model + its provider
  from `SettingsState` (thread a `Settings` clone into
  `generate_assistant_reply`, or read it in `send_chat_message` and pass down).
- Build the `genai::Client` from settings instead of `Client::builder().build()`:
  - **Custom endpoint** (`*Compatible` + `base_url`): register a
    `ServiceTargetResolver` mapping the model to the provider's endpoint/adapter.
  - **Auth**: register an `AuthResolver` that returns the keychain key when
    present; when absent, let `genai` fall back to its default env-var lookup
    (this gives "env by default, UI override optional" for free).
  - Apply `max_tokens` / `max_output_tokens` via `ChatOptions`.
- Keep `genai` calls confined to `generation.rs` (per AGENTS.md).
- Exact resolver signatures to confirm against `genai 0.6.5` during
  implementation.

> Note: this generation rewiring is the largest backend risk. It can land as a
> **second phase** — phase 1 can persist settings and keep the existing const
> model so the UI/plumbing ships independently.

### 2.4 Regenerate bindings

Run `pnpm tauri-typegen` after commands/types compile; then clean up
`src/generated/` per the `tauri-typegen` skill (unused imports/symbols).

### 2.5 Live model listing (shipped as `all_model_names`)

Live model listing is a dedicated command rather than a document field:

```rust
#[tauri::command]
async fn all_model_names(
    kind: AdapterKind,
    base_url: Option<String>,
    key: Option<String>,   // placeholder or omitted => resolve from `id`
    id: Option<String>,
    state,
) -> Result<Vec<String>, String>
```

It builds a `genai::ProviderConfig` (`Endpoint::from_owned(base_url)` +
`AuthData::from_single(key)`) and calls `genai::Client::all_model_names(kind,
config)`. For most adapters this is a live `{base_url}models` request (needs a
key); Ollama queries its host. The FE calls it per provider on demand (a "Fetch
models" button) with its own loading/error state and merges the results into the
model picker. Note: the network call means listing can be slow/fallible — the FE
owns the loading/error UX.

> The key resolution (`key == placeholder && id.is_some()` → use the stored key)
> is what lets the settings form list models for a provider whose key it only
> ever holds as the placeholder.

---

## 3. Frontend

### 3.1 Settings store — `src/app/shared/settings-store.ts` (Zustand)

> **Implementation drift:** this whole section describes the originally
> planned "push content, merge without refetching" design. The shipped store
> lives at `src/app/shared/settings-store.ts` (not
> `src/app/settings/settings-store.ts`) and instead follows the
> category-tag-then-refetch design from the updated Events section:
>
> - `settings-updated` only carries `{ categories: SettingsCategory[] }`; the
>   store's `init()` handler reacts by calling `getSettingsByCategories` (or
>   `getSettings` when `subscribedCategories` is `null`) and merging the
>   response — it does not merge the event payload directly.
> - There are no `useUiSettings()` / `useProvidersSettings()` selector hooks or
>   `fetchEnvKeys` / `fetchModels` store actions / `envKeys` / `modelsByProvider`
>   caches. Instead:
>   - A single generic `useSettingsSlice<T extends keyof Settings>(property)`
>     hook returns `[Settings[T], (data) => Promise<void>]`, used for both `ui`
>     and `llmProviders`.
>   - Provider metadata (env-key presence, custom-model support) is fetched via
>     a separate `@tanstack/react-query` hook,
>     `useProvidersMeta()` (`src/app/settings/sections/use-providers-meta.ts`),
>     wrapping `getProviders()` — not a Zustand cache.
>   - Live model listing (`allModelNames` / "Fetch models") is **not yet
>     implemented** in the providers form.
>
> The subsections below are kept for historical context; treat store API names
> as illustrative, not authoritative.

Use **Zustand** (`create`) rather than the `chat-store.ts`
`useSyncExternalStore` pattern. Zustand's selector-based subscriptions give the
"subscribe to one category" behaviour for free: a consumer that reads
`useSettingsStore((s) => s.ui)` only re-renders when the `ui` slice changes.

The store holds a **single** persisted slice (there is no derived companion).
The provider `apiKey` field is part of that slice; the store only ever holds the
placeholder or empty string, never a real key.

Store shape:

- **State:**
  - `ui: UiSettings`, `providers: Provider[]` (each `provider.apiKey` is the
    placeholder when a key is stored, empty otherwise).
  - on-demand caches (optional, not part of the document): `envKeys:
    Record<AdapterKind, boolean>`, `modelsByProvider: Record<string, string[]>`
    populated by the dedicated commands.
  - `status` flag for the initial load.
- **Actions:**
  - `init()`: `getSettings()` once → seed the persisted slice. Then subscribe to
    `settings-updated` (keep the unlisten fn for cleanup) and **merge the changed
    sections directly** (`set((s) => ({ ...s, ...changed }))`). No refetch — the
    event carries the content (keys already redacted to the placeholder). This
    covers this window's own saves *and* another window's.
  - `save(update: SettingsUpdate)`: call the single `saveSettings` command. The
    resulting `settings-updated` event is what commits new values into the store
    (single source of truth), so `save` need not mutate state itself — optionally
    set an in-flight/`saving` flag for the button. **API keys ride inside the
    providers section**: to keep a stored key, leave `apiKey` as the placeholder;
    to change it, set the new value; to clear it, send an empty string.
  - `fetchEnvKeys(kinds)` / `fetchModels({ kind, baseUrl, key, id })`: thin
    wrappers over `providerEnvKeys` / `allModelNames`, called on demand by the
    providers form; cache into the optional maps above.
- **Selector hooks** (thin wrappers over the store for ergonomics):
  - `useUiSettings()` → `{ ui, save: (ui) => save({ ui }) }`.
  - `useProvidersSettings()` → `{ providers, save, fetchEnvKeys, fetchModels }`
    where `save(providers)` calls `save({ llmProviders: { providers } })`.

Because `save` and the `settings-updated` event both speak `SettingsUpdate`, a
single call can persist multiple sections at once (e.g. a future "reset all"
flow) and the store merges whatever the echoed event contains. Env-key presence
and live model lists never travel through `save`/`SettingsUpdate`; they are read
via `providerEnvKeys` / `allModelNames` on demand.

### 3.2 Dependencies

Add `zustand` (settings store) plus `react-hook-form`, `@hookform/resolvers`,
`yup` (section forms) to `package.json`.

### 3.3 Settings window — `src/app/settings-window.tsx`

Replace the `Splitter` with a **vertical `Tabs`** layout (`orientation="vertical"`):

- `Tabs.List` on the left = category list (UI, LLM Providers).
- `Tabs.Content` on the right = the section's form.
- Each section is a self-contained component with its own RHF form + Yup schema
  + Save button + dirty tracking; Save disabled until dirty; toast on
  success/error via existing `Toaster`.
- Unsaved-changes guard when switching tabs (confirm dialog via `Dialog`).

### 3.4 Sections

**UI section** — `src/app/settings/sections/ui-section.tsx`
- Fields: theme (`RadioGroup`/`Select`), font size (`NumberInput`).
- Save → `useUiSettings().save(ui)` → `saveSettings({ ui })`.

**Providers section** — `src/app/settings/sections/providers-section.tsx`
- `useFieldArray` over `providers`.
- Per provider (rendered as an `Accordion` item — provider→model nesting):
  - `name`, `kind` (`Select`), `enabled` (`Switch`).
  - **`baseUrl`** — the primary provider field (required for `*Compatible`,
    optional for native). `Input`.
  - **API key** — a plain `TextField` (`type="password"`) bound to the form's
    `providers.${index}.apiKey`, saved with the rest of the form (shipped as
    planned here). When a key is stored the field is pre-filled with the
    placeholder; leaving it untouched keeps the stored key, editing it sets a new
    key, clearing it removes the key. "Using environment key" hint comes from
    `useProvidersMeta()` (a `getProviders()` query), not `providerEnvKeys`. No
    separate Set/Clear buttons — the key saves with the form.
  - `kind` options are also sourced from `useProvidersMeta()` (`getProviders()`)
    rather than a static `ADAPTER_KINDS` list, so the dropdown only lists the
    adapter kinds the backend actually exposes via `PROVIDER_KINDS`.
  - Nested `useFieldArray` over `models` (the user's **custom** models):
    - `name`, **`maxTokens`**, **`maxOutputTokens`** (`NumberInput`), remove
      button. For native vendors, labelled "Additional models".
  - **Fetch models** — planned, but **not yet implemented**: there is no button
    calling `allModelNames` yet; models are currently only added/removed
    manually via the nested field array.
- Save → `useSettingsSlice('llmProviders')` → `saveSettings({ update: { llmProviders } })`.

### 3.5 Consumers

Wherever the app needs a setting (e.g. terminal font size), read it via a
Zustand selector (`useSettingsStore((s) => s.ui.fontSize)`) so it live-updates
on `settings-updated` and only re-renders on that slice.

---

## 4. Sequencing / PRs

1. **BE core:** `settings/` — `Settings` + `SettingsUpdate`, state
   (load/persist/`apply`) with the four providers mappers, `get_settings` +
   `save_settings` + the on-demand commands (`get_providers`,
   `all_model_names`, `new_provider_id`), and the single `settings-updated`
   event, keeping the existing const model. `cargo check`. **(done)**
2. **Typegen:** regenerate + clean up `src/generated/` (event payload is
   `SettingsUpdatedPayload { categories }`, not `SettingsUpdate` itself — see
   the Events drift note in §2.1; `DerivedSettings` / `ProviderDerived` /
   `ModelInfo` / `getDerivedSettings` / `set_provider_api_key` /
   `clear_provider_api_key` are gone; new commands `all_model_names` /
   `get_providers`).
3. **FE store:** `src/app/shared/settings-store.ts` (Zustand) + generic
   `useSettingsSlice`; a single persisted slice refetched by category via
   `settings-updated` (keys as placeholder) plus `useProvidersMeta()`
   (react-query) for on-demand provider metadata.
4. **FE window:** vertical Tabs + UI section (simplest) end-to-end to validate
   the save→persist→event→refetch loop (event signals changed categories;
   the store refetches them).
5. **FE providers section:** provider/model field arrays, `apiKey` field
   (placeholder semantics). Fetch-models button not yet implemented.
6. **Generation rewiring:** consume selected model/provider, custom endpoint +
   auth resolvers, per-model token options.
7. **Consumers:** wire at least one real UI setting (font size) as a
   subscription demo (`useSettingsStore((s) => s.ui.fontSize)`).

## 5. Open questions / assumptions

- UI-settings seed fields (theme, font size) are assumed; confirm the real first
  batch.
- Whether `selectedModel` lives in the providers section UI now or later.
- Confirm `genai 0.6.5` `ServiceTargetResolver` / `AuthResolver` signatures at
  implementation time.
- **Resolved:** provider kinds use genai's own `AdapterKind` directly; no
  souffleur-side `ProviderKind`/`NativeVendor`. The provider dropdown lists
  every compiled `AdapterKind` variant. `Mistral` (previously in `NativeVendor`)
  has no `AdapterKind` in genai 0.6.5; configure it as an `OpenAI` kind with a
  `base_url` if needed.
- **Resolved:** `base_url` is optional for all kinds; the old
  "compatible requires base URL" validation is dropped.
- **Resolved:** a single `save_settings(update: SettingsUpdate)` command
  replaces the per-section `update_ui_settings` / `update_providers`; it accepts
  1..n sections and saves them atomically.
- **Superseded:** the plan originally called for the `settings-updated` event
  payload to be a `SettingsUpdate` carrying the changed sections' content
  directly (removing `SettingsUpdatedPayload` / `SettingsCategory`), with the FE
  store merging the pushed content instead of refetching. The shipped design
  reverses this: the payload is `SettingsUpdatedPayload { categories:
  Vec<SettingsCategory> }` (a change signal only), and the FE store refetches
  the affected categories via `get_settings_by_categories`.
- **Resolved (revised):** there is **no** `DerivedSettings` document. `Settings`
  holds everything persisted, including the provider `api_key` (a real field
  backed by the keychain, not the file). Per-section **mappers** move each field
  across the four boundaries (read / store / send-to-FE / receive-from-FE); the
  FE only ever sees a placeholder for a stored key. This supersedes the earlier
  "split persisted vs. derived" design.
- **Resolved:** genuinely computed facets are **pulled on demand** via dedicated
  commands (`get_providers`, `all_model_names`), not carried on the document.
  There is no derived event: because keys ride inside the document as the
  placeholder, a normal `settings-updated` already signals a possible key change,
  and key set/clear/change now go through `save_settings` like any providers edit.
- **Resolved:** live (network) model listing is the `all_model_names` command
  (§2.5); its key argument resolves the stored key when given the placeholder + `id`.
- **Resolved:** the FE settings store uses **Zustand** (selector-based slice
  subscriptions), diverging from the `chat-store.ts` `useSyncExternalStore`
  pattern; it holds a single pushed persisted slice (keys as placeholder).
- Open: whether `save_settings` should be all-or-nothing across sections in one
  `update` (current assumption) or persist valid sections and report per-section
  errors. Assuming all-or-nothing for simplicity.

## 6. Testing

- Rust unit tests in `settings/`: default load, round-trip serialize, atomic
  write, validation rejections. Mapper coverage (in `llm_providers.rs`): the file
  strip removes any key (assert no `apiKey`/secret in the JSON), redact replaces
  a stored key with the placeholder (and leaves keyless providers `None`),
  reconcile keeps the stored key on placeholder / clears on empty / takes a new
  value otherwise, and `get_providers` maps requested kinds. `save_settings`
  applies only the present sections (a `ui`-only update leaves `llm_providers`
  untouched and vice-versa) and a multi-section update applies both. *(These are
  implemented and passing.)*
- `cargo check` in `src-tauri/`; `pnpm build` (tsc) for the frontend.
- Manual: open settings, edit a section, Save, observe toast + that a subscribed
  consumer updates without a window reload (driven by the pushed event content).
  For keys: set a key, confirm the field shows the placeholder on reload and the
  raw key never appears in `settings.json`.
