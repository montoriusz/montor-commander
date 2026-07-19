import type { SettingsCategory } from '@/generated';

export const QUERY_KEY_LLM_PROVIDERS = ['llmProviders'];

/** Groupings from data derived from single settings category (TODO: evaluate and revisit approach when more queries exist) */
export const QUERY_KEY_SETTINGS_CATEGORY_BASED = ['settings'];

export const getQueryKeyForSettingsCategory = (category: SettingsCategory) => [
  ...QUERY_KEY_SETTINGS_CATEGORY_BASED,
  category,
];

/** Models available for chat, grouped by provider (see `list_available_models`). */
export const QUERY_KEY_LLM_MODELS = [
  ...getQueryKeyForSettingsCategory('LlmProviders'),
  'llmModels',
] as const;
