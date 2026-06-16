import {
  type ChatMessage,
  type ChatPage,
  getChatSession,
  onChatGenerationError,
  onChatMessagesChanged,
  readChatMessages,
  sendChatMessage,
} from '@/generated';

interface ChatState {
  messages: ChatMessage[];
  cursor: string | null;
  isGenerating: boolean;
  error: string | null;
}

type Listener = () => void;

const state: ChatState = {
  messages: [],
  cursor: null,
  isGenerating: false,
  error: null,
};

const listeners = new Set<Listener>();

function notify() {
  for (const listener of listeners) {
    listener();
  }
}

function getSnapshot(): ChatState {
  return state;
}

function subscribe(listener: Listener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

async function init() {
  try {
    // Ensure chat session is initialised.
    await getChatSession();

    // Initial pull — load all messages from the beginning.
    const page: ChatPage = await readChatMessages({ afterCursor: null });
    state.messages = page.messages;
    state.cursor = page.nextCursor ?? null;

    // Register event listeners.
    onChatMessagesChanged((_payload) => {
      void pull();
    });

    onChatGenerationError((payload) => {
      state.error = payload.message;
      state.isGenerating = false;
      notify();
    });
  } catch (e) {
    console.error('chat-store init failed:', e);
  }
}

async function pull() {
  try {
    const page: ChatPage = await readChatMessages({
      afterCursor: state.cursor,
    });
    if (page.messages.length === 0) return;

    // Dedupe by id
    const existingIds = new Set(state.messages.map((m) => m.id));
    const toAppend = page.messages.filter((m) => !existingIds.has(m.id));

    if (toAppend.length > 0) {
      state.messages.push(...toAppend);
    }

    state.cursor = page.nextCursor ?? state.cursor;

    // If any appended message is an assistant message, generation is done.
    if (toAppend.some((m) => m.type === 'Assistant')) {
      state.isGenerating = false;
    }

    notify();
  } catch (e) {
    console.error('chat-store pull failed:', e);
  }
}

async function send(text: string) {
  state.isGenerating = true;
  state.error = null;
  notify();

  try {
    await sendChatMessage({ text });
    // New messages arrive via the pull triggered by the BE event.
  } catch (e) {
    state.error = String(e);
    state.isGenerating = false;
    notify();
  }
}

// Initialise on module load (mirrors the "global, outside component" intent).
void init();

export const chatStore = { getSnapshot, subscribe, send };
