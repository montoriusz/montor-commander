import { Terminal } from 'xterm';
import { FitAddon } from 'xterm-addon-fit';
import 'xterm/css/xterm.css';
import { invoke } from '@tauri-apps/api/core';

const terminalElement = document.getElementById('terminal') as HTMLElement;

const fitAddon = new FitAddon();
const term = new Terminal({
  fontFamily: 'Jetbrains Mono',
  theme: {
    background: '#181818',
  },
});
term.loadAddon(fitAddon);
term.open(terminalElement);

// Make the terminal fit all the window size
async function fitTerminal() {
  fitAddon.fit();
  void invoke<string>('resize_pty', {
    rows: term.rows,
    cols: term.cols,
  });
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

// Write data from the terminal to the pty
function writeToPty(data: string) {
  void invoke('write_to_pty', {
    data,
  });
}
function initShell() {
  invoke('create_shell').catch((error) => {
    console.error('Error creating shell:', error);
  });
}

initShell();
term.onData(writeToPty);
addEventListener('resize', fitTerminal);
fitTerminal();

async function readFromPty() {
  const data = await invoke<string>('read_from_pty');

  if (data) {
    await writeToTerminal(data);
  }

  window.requestAnimationFrame(readFromPty);
}

window.requestAnimationFrame(readFromPty);
