import { useEffect } from 'react';
import { type Control, type FieldValues, useFormState } from 'react-hook-form';
import { create } from 'zustand';

/**
 * Tracks per-tab settings form status — dirty flag and validation errors —
 * so the settings window can surface markers on tab labels even when the
 * form owning the tab is unmounted (the sections are lazy/Suspense-loaded).
 *
 * Section forms report their status via {@link useSettingsFormStatusSync};
 * the settings window reads it via {@link useSettingsFormStatus}.
 */
export type SettingsTabId = 'ui' | 'providers';

export interface SettingsFormStatus {
  dirty: boolean;
  hasErrors: boolean;
}

interface SettingsFormStatusState {
  status: Record<SettingsTabId, SettingsFormStatus>;
  setStatus: (id: SettingsTabId, status: SettingsFormStatus) => void;
  reset: (id: SettingsTabId) => void;
}

const EMPTY_STATUS: SettingsFormStatus = { dirty: false, hasErrors: false };

const useSettingsFormStatusStore = create<SettingsFormStatusState>((set) => ({
  status: { ui: { ...EMPTY_STATUS }, providers: { ...EMPTY_STATUS } },

  setStatus: (id, next) =>
    set((state) => {
      const prev = state.status[id];
      if (prev.dirty === next.dirty && prev.hasErrors === next.hasErrors) return state;
      return { status: { ...state.status, [id]: next } };
    }),

  reset: (id) => set((state) => ({ status: { ...state.status, [id]: { ...EMPTY_STATUS } } })),
}));

/** Selector returning a single tab's status, scoped to render-on-change. */
export function useSettingsFormStatus(id: SettingsTabId): SettingsFormStatus {
  return useSettingsFormStatusStore((s) => s.status[id]);
}

/**
 * Uniform hook that mirrors a `react-hook-form` instance's `isDirty` and
 * `isValid` into the per-tab status store. Use once per section form.
 *
 * `isValid` is `false` until the resolver first runs, so we gate the error
 * flag on `isDirty` — a pristine, freshly loaded form never shows an error
 * marker.
 */
export function useSettingsFormStatusSync<T extends FieldValues>(
  id: SettingsTabId,
  control: Control<T>,
) {
  const setStatus = useSettingsFormStatusStore((s) => s.setStatus);
  const { isDirty, isValid } = useFormState({ control });

  useEffect(() => {
    setStatus(id, { dirty: isDirty, hasErrors: isDirty && !isValid });
  }, [id, setStatus, isDirty, isValid]);
}
