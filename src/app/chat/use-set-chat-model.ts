import { useMutation, useQueryClient } from '@tanstack/react-query';
import { setChatModel } from '@/generated';
import { QUERY_KEY_CHAT_SESSION } from '../shared/query-keys';

/**
 * Set the model used for the current chat session. Pass `null` (or omit the
 * alias) to revert to the default selection (first enabled primary provider's
 * first model).
 *
 * On success invalidates the chat-session query so the dropdown reflects the
 * newly-active item (`ChatSessionInfo.model`).
 */
export const useSetChatModel = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (alias: string | null) => setChatModel({ alias }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: QUERY_KEY_CHAT_SESSION });
    },
  });
};
