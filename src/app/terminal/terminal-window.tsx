import { useRef } from 'react';
import { Splitter } from '@/ui/primitives';
import { ChatPane } from '../chat';

import { type TerminalHandle, TerminalPane } from '../terminal';
import { SectionMatching, UpdateMatchingProvider, useEmitUpdateMatching } from './section-matching';

const PANELS: Splitter.PanelData[] = [
  { id: 'terminal', minSize: '30rem' },
  { id: 'chat', minSize: '18rem', maxSize: '50%' },
];

function TerminalWindowInner() {
  const splitterRootRef = useRef<HTMLDivElement>(null);
  const terminalRef = useRef<TerminalHandle>(null);
  const emitUpdateMatching = useEmitUpdateMatching();

  return (
    <Splitter.Root
      ref={splitterRootRef}
      panels={PANELS}
      variant="full"
      defaultSize={[5, 3]}
      orientation="horizontal"
      h="screen"
      w="screen"
      position="relative"
      onResizeEnd={() => {
        terminalRef.current?.fit();
        emitUpdateMatching();
      }}
    >
      <Splitter.Panel id="terminal" h="full">
        <TerminalPane ref={terminalRef} />
      </Splitter.Panel>
      <Splitter.ResizeTrigger id="terminal:chat" aria-label="Resize panes">
        <Splitter.ResizeTriggerIndicator />
      </Splitter.ResizeTrigger>
      <Splitter.Panel id="chat" h="full" borderWidth="0">
        <ChatPane />
      </Splitter.Panel>
      <SectionMatching />
    </Splitter.Root>
  );
}

export function TerminalWindow() {
  return (
    <UpdateMatchingProvider>
      <TerminalWindowInner />
    </UpdateMatchingProvider>
  );
}
