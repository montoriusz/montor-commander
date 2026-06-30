import {
  type ChatMessage,
  type ChatPage,
  getChatSession,
  onChatGenerationError,
  onChatMessagesChanged,
  readChatMessages,
  sendChatMessage,
} from '@/generated';
import { debounce } from '../shared/debounce';
import { commandlineController } from '../terminal';

interface ChatState {
  messages: ChatMessage[];
  cursor: string | undefined;
  isGenerating: boolean;
  error: string | undefined;
}

type Listener = () => void;

let state: ChatState = {
  messages: [],
  cursor: undefined,
  isGenerating: false,
  error: undefined,
};

const PULL_DEBOUNCE_MS = 100;

function setState(patch: Partial<ChatState>) {
  state = { ...state, ...patch };
}

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
    setState({
      messages: page.messages,
      cursor: page.nextCursor ?? undefined,
    });

    // Register event listeners.
    const debouncedPull = debounce(() => void pull(), PULL_DEBOUNCE_MS);
    onChatMessagesChanged(() => debouncedPull());

    onChatGenerationError((payload) => {
      setState({
        error: payload.message,
        isGenerating: false,
      });
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

    if (toAppend.length === 0) return;

    setState({
      messages: [...state.messages, ...toAppend],
      cursor: page.nextCursor ?? state.cursor,
      isGenerating: toAppend.some((m) => m.type === 'Assistant') ? false : state.isGenerating,
    });

    notify();

    const lastMessage = toAppend.at(-1);
    if (lastMessage?.type === 'Assistant' && lastMessage?.cmdline) {
      commandlineController.put(lastMessage.cmdline);
    }
  } catch (e) {
    console.error('chat-store pull failed:', e);
  }
}

async function send(msg: string) {
  setState({ isGenerating: true, error: undefined });
  notify();

  try {
    await sendChatMessage({ msg });
    // New messages arrive via the pull triggered by the BE event.
  } catch (e) {
    setState({ error: String(e), isGenerating: false });
    notify();
  }
}

// TODO: dispose previous instance
void init();

export const chatStore = { getSnapshot, subscribe, send };
