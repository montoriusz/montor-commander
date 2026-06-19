import {
  type ChatMessage,
  type ChatPage,
  getChatSession,
  onChatGenerationError,
  onChatMessagesChanged,
  readChatMessages,
  type SendChatMessageParams,
  sendChatMessage,
} from '@/generated';
import { terminalSections } from '../terminal';
import type { SectionSnapshot } from '../terminal/terminal-sections';

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
    let pullTimer: ReturnType<typeof setTimeout> | undefined;
    onChatMessagesChanged((_payload) => {
      if (pullTimer != null) clearTimeout(pullTimer);
      pullTimer = setTimeout(() => {
        pullTimer = undefined;
        void pull();
      }, PULL_DEBOUNCE_MS);
    });

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
    console.log('pulling chat messages', { cursor: state.cursor });
    const page: ChatPage = await readChatMessages({
      afterCursor: state.cursor,
    });
    console.log('pulled chat messages', {
      nextCursor: page.nextCursor,
      count: page.messages.length,
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
  } catch (e) {
    console.error('chat-store pull failed:', e);
  }
}

async function send(msg: string) {
  setState({ isGenerating: true, error: undefined });
  notify();

  const lastMarker = terminalSections.getLastSectionId();

  const previousMarker = (
    state.messages.findLast((message) => {
      return (
        message.type === 'User' &&
        message.terminal_marker != null &&
        message.terminal_marker !== lastMarker
      );
    }) as { terminal_marker: string } | undefined
  )?.terminal_marker;

  const sections = terminalSections.getSectionShapshots(previousMarker);

  const payload: SendChatMessageParams['payload'] = {
    terminalMarker: lastMarker,
  };
  if (msg) payload.msg = msg;

  const lastSection = sections.at(-1);
  if (lastSection !== undefined && lastSection.id === terminalSections.getLastSectionId()) {
    payload.commandline = lastSection.command;
  }

  if (sections.length) {
    payload.terminal = formatTerminalSections(sections);
  }

  try {
    await sendChatMessage({ payload });
    // New messages arrive via the pull triggered by the BE event.
  } catch (e) {
    setState({ error: String(e), isGenerating: false });
    notify();
  }
}

function formatTerminalSections(sections: SectionSnapshot[]): string {
  return sections
    .reduce((acc, section) => {
      const isSectionExecuted = section.output !== undefined;
      return `${acc}
<prompt>
${section.prompt}
</prompt>${
        isSectionExecuted
          ? `
<command>
${section.command ?? ''}
</command>
<output>
${section.output ?? ''}
</output>`
          : ''
      }`;
    }, '')
    .trim();
}

void init();

export const chatStore = { getSnapshot, subscribe, send };
