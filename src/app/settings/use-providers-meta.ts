import { useQuery } from '@tanstack/react-query';
import { QUERY_KEY_LLM_PROVIDERS } from '@/app/shared/query-keys';
import { getProviders } from '@/generated';

export const useProvidersMeta = () => {
  return useQuery({
    queryKey: QUERY_KEY_LLM_PROVIDERS,
    queryFn: getProviders,
  });
};
