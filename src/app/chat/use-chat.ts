import { useSyncExternalStore } from 'react';
import { chatStore } from './chat-store';
import type { ChatMessage } from '@/generated';

export function useChat() {
  const state = useSyncExternalStore(chatStore.subscribe, chatStore.getSnapshot);

  return {
    messages: state.messages as readonly ChatMessage[],
    isGenerating: state.isGenerating,
    error: state.error,
    send: chatStore.send,
  };
}
