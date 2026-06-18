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

async function send(msg: string) {
  state.isGenerating = true;
  state.error = null;
  notify();

  const previousMarker =
    state.messages.findLast((message) => message.type === 'User')?.terminal_marker ?? undefined;

  const lastExecutedMarker = terminalSections.getLastExecutedSectionId();

  const sections = terminalSections.getSectionShapshots(previousMarker);

  const payload: SendChatMessageParams['payload'] = {};
  if (msg) payload.msg = msg;
  if (lastExecutedMarker) payload.terminalMarker = lastExecutedMarker;

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
    state.error = String(e);
    state.isGenerating = false;
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
