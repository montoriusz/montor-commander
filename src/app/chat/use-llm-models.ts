import { useQuery } from '@tanstack/react-query';
import { listAvailableModels } from '@/generated';
import { QUERY_KEY_LLM_MODELS } from '../shared/query-keys';

/**
 * Models available for the chat model dropdown, grouped by provider.
 *
 * The backend (`list_available_models`) live-fetches Ollama models via genai;
 * other providers return their configured model list. Disabled/unaddressable
 * providers are skipped server-side. Failures per provider degrade to an empty
 * group (logged server-side) rather than failing the whole query.
 */
export const useLlmModels = () => {
  return useQuery({ queryKey: QUERY_KEY_LLM_MODELS, queryFn: listAvailableModels });
};
