import { useMutation } from '@tanstack/react-query';
import { type AllModelNamesParams, allModelNames } from '@/generated';

/**
 * On-demand "Fetch models" for a provider. A mutation (not a `useQuery`) is the
 * right primitive: the result is transient input for a form merge, and the
 * request params (`baseUrl`/`key`/`id`) can change between clicks, so a cache
 * key would be misleading. Loading/error state is owned by Query — the form
 * has no hand-rolled `fetching` flag.
 *
 * `key` may be the redacted placeholder; the backend resolves the stored key
 * from `id` in that case (see §2.5 of `implementation-plans/settings.md`), so
 * the caller should pass `id` wherever it has one.
 */
export const useFetchModels = () => {
  return useMutation({
    mutationFn: (params: AllModelNamesParams) => allModelNames(params),
  });
};
