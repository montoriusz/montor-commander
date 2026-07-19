import { useCallback } from 'react';
import { create } from 'zustand';
import {
  getSettings,
  getSettingsByCategories,
  type LlmProviderSettings,
  onSettingsUpdated,
  type Settings,
  type SettingsCategory,
  type SettingsUpdate,
  saveSettings,
  type UiSettings,
} from '@/generated';

// ---------------------------------------------------------------------------
// Store types
// ---------------------------------------------------------------------------

export interface SettingsStoreState extends Settings {
  subscribedCategories: SettingsCategories;
  initialized: boolean;
}

export const ALL_SETTINGS_CATEGORIES = Symbol('ALL_SETTINGS_CATEGORIES');

type AllCats = typeof ALL_SETTINGS_CATEGORIES;
export type SettingsCategories = SettingsCategory[] | AllCats;

interface SettingsActions {
  /** Seed both slices once, then subscribe to `settings-updated`. */
  init: (subscribeToCategories: SettingsCategory[] | AllCats) => Promise<void>;
  /** Persist 1..n sections; the resulting `settings-updated` event commits them. */
  save: (update: SettingsUpdate) => Promise<void>;
}

export interface SettingsStore extends SettingsStoreState, SettingsActions {}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async function readPartial(categories: SettingsCategory[] | typeof ALL_SETTINGS_CATEGORIES) {
  let data: Partial<Settings>;
  if (categories === ALL_SETTINGS_CATEGORIES) {
    data = await getSettings();
  } else {
    const payload = await getSettingsByCategories({ categories });
    data = Object.fromEntries(
      Object.entries(payload)
        .values()
        .filter(([_key, value]) => value != null),
    );
  }
  return data;
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

const defaultUi: UiSettings = { theme: 'system', fontSize: 13, uiScale: 100 };
const defaultLlmProviders: LlmProviderSettings = { providers: [] };

export const useSettingsStore = create<SettingsStore>((set, get) => ({
  // Persisted slice
  ui: defaultUi,
  llmProviders: defaultLlmProviders,
  initialized: false,
  subscribedCategories: [],

  init: async (subscribeToCategories) => {
    const { initialized } = get();
    if (initialized) return;

    set({ subscribedCategories: subscribeToCategories });

    try {
      const snapshot = await readPartial(subscribeToCategories);
      set({ ...snapshot, initialized: true });

      await onSettingsUpdated(async (updated) => {
        const subscribedCat = get().subscribedCategories;
        const updatedCat = new Set(updated.categories);
        const catToFetch =
          subscribedCat === ALL_SETTINGS_CATEGORIES
            ? updated.categories
            : subscribedCat.filter((cat) => updatedCat.has(cat));

        if (catToFetch.length !== 0) {
          set(await readPartial(catToFetch));
        }
      });
    } catch (e) {
      console.error('settings-store init failed:', e);
    }
  },

  save: async (update: SettingsUpdate) => {
    await saveSettings({ update });
  },
}));

// ---------------------------------------------------------------------------
// Init (once per app session)
// ---------------------------------------------------------------------------

export const initSettingStore = (subscribeToCategories: SettingsCategory[] | AllCats) =>
  useSettingsStore.getState().init(subscribeToCategories);

// ---------------------------------------------------------------------------
// Selector hook
// ---------------------------------------------------------------------------
export function useSettingsSlice<T extends keyof Settings>(
  property: T,
): [Settings[T], (data: Settings[T]) => Promise<void>] {
  const data = useSettingsStore((s) => s[property]);
  const save = useSettingsStore((s) => s.save);

  const saveSlice = useCallback(
    (data: Settings[T]) => save({ [property]: data }),
    [save, property],
  );

  return [data, saveSlice];
}
