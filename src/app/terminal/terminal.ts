import { listen } from '@tauri-apps/api/event';
import { FitAddon } from '@xterm/addon-fit';
import { Terminal } from '@xterm/xterm';
import { token } from 'styled-system/tokens';
import { createShell, resizePty, writeToPty } from '@/generated';
import { TerminalSections } from './terminal-sections';

const terminal = new Terminal({
  fontFamily: 'JetBrains Mono',
  theme: {
    background: token('colors.canvas'),
    green: '#33b074',
    brightGreen: '#3dd68c',
    blue: '#3b9eff',
    brightBlue: '#70b8ff',
    overviewRulerBorder: '#272a29',
  },
  allowProposedApi: true,
  overviewRuler: {
    width: 12,
  },
});

const fitAddon = new FitAddon();
terminal.loadAddon(fitAddon);

const terminalSections = new TerminalSections();
terminal.loadAddon(terminalSections);

// TODO: convert to channel
const unlistenPromise = listen<{ data: string }>('pty-output', (e) => {
  terminal.write(e.payload.data);
});

terminal.onData((data) => {
  void writeToPty({ data });
});

createShell().catch((error) => {
  console.error('Error creating shell:', error);
});

function fitTerminal() {
  fitAddon.fit();
  void resizePty({ rows: terminal.rows, cols: terminal.cols });
}

function unlistenTerminal() {
  void unlistenPromise.then((unlisten) => unlisten());
}

const commandlineController = {
  put: (command: string) => {
    void writeToPty({ data: `\x05\x15${command}` });
  },
  putAndExecute: (command: string) => {
    void writeToPty({ data: `\x05\x15${command}\r` });
  },
};

export { commandlineController, fitTerminal, terminal, terminalSections, unlistenTerminal };
