import { listen } from '@tauri-apps/api/event';
import { FitAddon } from '@xterm/addon-fit';
import { Terminal } from '@xterm/xterm';
import { forwardRef, useCallback, useEffect, useImperativeHandle, useRef } from 'react';
import { Box } from 'styled-system/jsx';
import { token } from 'styled-system/tokens';
import { createShell, resizePty, writeToPty } from '@/generated';
import {
  createSection,
  type MarkingPoint,
  parseOsc133,
  type Section,
  updateSectionDecorations,
} from './terminal-sections';

import '@xterm/xterm/css/xterm.css';

export interface TerminalHandle {
  fit: () => void;
}

export const TerminalPane = forwardRef<TerminalHandle>(function TerminalPane(_, handleRef) {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const fitTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const fit = useCallback(() => {
    if (fitTimerRef.current != null) clearTimeout(fitTimerRef.current);
    fitTimerRef.current = setTimeout(() => {
      const term = termRef.current;
      const addon = fitAddonRef.current;
      if (!term || !addon) return;
      addon.fit();
      void resizePty({ rows: term.rows, cols: term.cols });
    }, 100);
  }, []);

  useImperativeHandle(handleRef, () => ({ fit }), [fit]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    // TODO: Detach Terminal and tauri inter-op from Component, initialize in the global scope
    const term = new Terminal({
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
    termRef.current = term;

    const fitAddon = new FitAddon();
    fitAddonRef.current = fitAddon;
    term.loadAddon(fitAddon);
    term.open(container);

    const sections = new Map<string, Section>();
    let lastAid = '';

    function registerMarkingPoint(markingPoint: MarkingPoint, aid?: string) {
      const effectiveAid = aid || lastAid;
      if (!effectiveAid) return;
      lastAid = effectiveAid;

      let section = sections.get(effectiveAid);
      if (!section) {
        section = createSection(effectiveAid);
        sections.set(effectiveAid, section);
      }

      section.markers[markingPoint] = {
        x: term.buffer.active.cursorX,
        y: term.buffer.active.cursorY + term.buffer.active.baseY,
      };
      updateSectionDecorations(term, section);
    }

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
      }
      return true;
    });

    const unlistenPromise = listen<{ data: string }>('pty-output', (e) => {
      term.write(e.payload.data);
    });

    createShell().catch((error) => {
      console.error('Error creating shell:', error);
    });

    term.onData((data) => {
      void writeToPty({ data });
    });

    fit();
    window.addEventListener('resize', fit);

    return () => {
      window.removeEventListener('resize', fit);
      if (fitTimerRef.current != null) clearTimeout(fitTimerRef.current);
      term.dispose();
      termRef.current = null;
      fitAddonRef.current = null;
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, [fit]);

  return (
    <Box
      ref={containerRef}
      h="full"
      w="full"
      py="0.5"
      pl="1.5"
      bg="canvas"
      borderRadius="l3"
      overflow="hidden"
    />
  );
});
