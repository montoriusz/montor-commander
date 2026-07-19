export const QUERY_KEY_LLM_PROVIDERS = ['llmProviders'];

/** Models available for chat, grouped by provider (see `list_available_models`). */
export const QUERY_KEY_LLM_MODELS = ['llmModels'] as const;

/** Current chat session info (id + selected model alias). */
export const QUERY_KEY_CHAT_SESSION = ['chatSession'] as const;
