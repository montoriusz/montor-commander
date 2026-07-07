/**
 * Yup schema for the UI settings form. Mirrors the backend validation in
 * `src-tauri/src/settings.rs` so the user gets inline feedback before the
 * round trip, and the BE rejection message is the rare edge case.
 */

import * as yup from 'yup';
import type { Settings } from '@/generated';

export type UiFormValues = Settings['ui'];

export const uiSettingsSchema: yup.ObjectSchema<UiFormValues> = yup.object({
  theme: yup.mixed<'system' | 'light' | 'dark'>().oneOf(['system', 'light', 'dark']).required(),
  fontSize: yup
    .number()
    .integer()
    .min(6, 'Font size must be at least 6')
    .max(72, 'Font size must be at most 72')
    .required(),
  uiScale: yup
    .number()
    .integer()
    .min(50, 'UI scale must be at least 50%')
    .max(200, 'UI scale must be at most 200%')
    .required(),
});
