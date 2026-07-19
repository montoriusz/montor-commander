import { QueryClient } from '@tanstack/react-query';
import { onSettingsUpdated } from '@/generated';
import { getQueryKeyForSettingsCategory } from './shared/query-keys';

export const queryClient = new QueryClient();

onSettingsUpdated((payload) => {
  payload.categories.forEach((category) => {
    queryClient.invalidateQueries({
      queryKey: getQueryKeyForSettingsCategory(category),
      exact: false,
    });
  });
});
