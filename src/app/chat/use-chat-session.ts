import { useQuery } from '@tanstack/react-query';
import { getChatSession } from '@/generated';
import { QUERY_KEY_CHAT_SESSION } from '../shared/query-keys';

/**
 * Current chat session info — used to read the selected model alias without
 * touching the chat-store. The backend populates `model` from
 * `ChatSession::selected_model`, so reading it here lets the dropdown mark its
 * active item and lets a default-initialized session show the default model.
 */
export const useChatSession = () => {
  return useQuery({ queryKey: QUERY_KEY_CHAT_SESSION, queryFn: getChatSession });
};
