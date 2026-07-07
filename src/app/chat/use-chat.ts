import { useSyncExternalStore } from 'react';
import type { ChatMessage } from '@/generated';
import { chatStore } from './chat-store';

// TODO: refactor to Zustand

export function useChat() {
  const state = useSyncExternalStore(chatStore.subscribe, chatStore.getSnapshot);

  return {
    messages: state.messages as readonly ChatMessage[],
    isGenerating: state.isGenerating,
    error: state.error,
    send: chatStore.send,
  };
}
