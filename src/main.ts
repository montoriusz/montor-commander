import { Terminal, type IDecoration } from 'xterm';
import { FitAddon } from 'xterm-addon-fit';
import 'xterm/css/xterm.css';
import { listen } from '@tauri-apps/api/event';
import { createShell, resizePty, writeToPty } from './generated';

const terminalElement = document.getElementById('terminal') as HTMLElement;

const fitAddon = new FitAddon();
const term = new Terminal({
  fontFamily: 'JetBrains Mono',
  theme: {
    background: '#181818',
  },
  allowProposedApi: true,
  overviewRulerWidth: 32,
});
term.loadAddon(fitAddon);
term.open(terminalElement);

(window as any).term = term;

interface BufferMarker {
  x: number;
  y: number;
}

type MarkingPoint = 'PromptStart' | 'PromptEnd' | 'CommandStart' | 'CommandEnd';

interface Section {
  id: string;
  markers: Record<MarkingPoint, BufferMarker | undefined>;
  promptDecoration: IDecoration | undefined;
  // assistentPrompt: string;
  // assistentResponse: string;
  // commandSuggestion: string;
  // command: string;
}

function createSection(id: string): Section {
  return {
    id,
    markers: {
      PromptStart: undefined,
      PromptEnd: undefined,
      CommandStart: undefined,
      CommandEnd: undefined,
    },
    promptDecoration: undefined,
  };
}

function updateSectionDecorations(
  term: Terminal,
  section: Section,
  force = false,
) {
  if (
    (!section.promptDecoration || force) &&
    section.markers.PromptStart &&
    section.markers.PromptEnd &&
    section.markers.CommandStart
  ) {
    if (section.promptDecoration) {
      section.promptDecoration.dispose();
      section.promptDecoration = undefined;
    }

    const marker = term.registerMarker(
      section.markers.PromptStart.y -
        term.buffer.active.cursorY -
        term.buffer.active.baseY,
    );
    if (marker) {
      const decoration = term.registerDecoration({
        marker,
        x: section.markers.PromptStart.x,
        width: term.cols - section.markers.PromptStart.x,
        layer: 'top',
        backgroundColor: '#440000',
        overviewRulerOptions: {
          color: '#440000',
          position: 'left',
        },
      });
      if (decoration) {
        section.promptDecoration = decoration;
        decoration.onRender((element: HTMLElement) => {
          element.style.borderTop = '1px solid #ff0000';
        });
        decoration.onDispose(() => {
          decoration.marker.dispose();
          section.promptDecoration = decoration;
        });
      } else {
        marker.dispose();
      }
    }
  }
}

const sections = new Map<string, Section>();

// Make the terminal fit all the window size
async function fitTerminal() {
  fitAddon.fit();
  void resizePty({ rows: term.rows, cols: term.cols });
}

// Write data from pty into the terminal
function writeToTerminal(data: string) {
  return new Promise<void>((r, reject) => {
    try {
      term.write(data, () => r());
    } catch (e) {
      reject(e);
    }
  });
}

let lastAid = '';
function registerMarkingPoint(markingPoint: MarkingPoint, aid?: number) {
  const aidStr = aid != null ? String(aid) : lastAid;

  if (!aidStr) return;
  lastAid = aidStr;

  let section = sections.get(aidStr);

  if (!section) {
    section = createSection(aidStr);
    sections.set(aidStr, section);
  }

  section.markers[markingPoint] = {
    x: term.buffer.active.cursorX,
    y: term.buffer.active.cursorY + term.buffer.active.baseY,
  };
  updateSectionDecorations(term, section);
}

// Parse OSC 133 data into marker, exitCode, and aid.
function parseOsc133(data: string): {
  marker: string;
  exitCode: number | undefined;
  aid: number | undefined;
} {
  const parts = data.split(';');
  const marker = parts[0];
  let exitCode: number | undefined;
  let aid: number | undefined;

  for (const part of parts.slice(1)) {
    if (part.startsWith('aid=')) {
      aid = parseInt(part.slice(4), 10);
    } else if (marker === 'D' && exitCode === undefined) {
      exitCode = parseInt(part, 10);
    }
  }

  return { marker, exitCode, aid };
}

// Register OSC 133 shell integration markers.
// The handler runs synchronously during term.write() parsing, so cursor
// position is correct without any setTimeout deferral.
term.parser.registerOscHandler(133, (data: string) => {
  const { marker, aid } = parseOsc133(data);

  if (marker === 'A') {
    registerMarkingPoint('PromptStart', aid);
  } else if (marker === 'B') {
    registerMarkingPoint('PromptEnd', aid);
  } else if (marker === 'C') {
    registerMarkingPoint('CommandStart', aid);
  } else if (marker === 'D') {
    registerMarkingPoint('CommandEnd', aid);
    // console.debug('command finished, aid:', aid, 'exit code:', exitCode ?? -1);
  }
  return true;
});

// Register listeners before starting the shell so no early output is missed.
await listen<{ data: string }>('pty-output', (e) => {
  void writeToTerminal(e.payload.data);
});

function initShell() {
  createShell().catch((error) => {
    console.error('Error creating shell:', error);
  });
}

initShell();
term.onData((data) => writeToPty({ data }));
addEventListener('resize', fitTerminal);
fitTerminal();
