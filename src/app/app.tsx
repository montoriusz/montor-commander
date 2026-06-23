import { useRef } from 'react';
import {
  ResizeTriggerIndicator,
  Panel as SplitterPanel,
  ResizeTrigger as SplitterResizeTrigger,
  Root as SplitterRoot,
} from '@/ui/primitives/splitter';
import { ChatPane } from './chat';
import { SectionMatching, UpdateMatchingProvider, useEmitUpdateMatching } from './section-matching';
import { type TerminalHandle, TerminalPane } from './terminal';

function AppInner() {
  const splitterRootRef = useRef<HTMLDivElement>(null);
  const terminalRef = useRef<TerminalHandle>(null);
  const emitUpdateMatching = useEmitUpdateMatching();

  return (
    <SplitterRoot
      ref={splitterRootRef}
      panels={[{ id: 'terminal' }, { id: 'chat' }]}
      variant="full"
      defaultSize={[5, 4]}
      orientation="horizontal"
      // TODO: move to recipe
      h="screen"
      w="screen"
      bg="border"
      p="[3px]"
      position="relative"
      onResizeEnd={() => {
        terminalRef.current?.fit();
        emitUpdateMatching();
      }}
    >
      <SplitterPanel id="terminal" h="full">
        <TerminalPane ref={terminalRef} />
      </SplitterPanel>
      <SplitterResizeTrigger id="terminal:chat" aria-label="Resize panes">
        <ResizeTriggerIndicator />
      </SplitterResizeTrigger>
      <SplitterPanel id="chat" h="full">
        <ChatPane />
      </SplitterPanel>
      <SectionMatching />
    </SplitterRoot>
  );
}

export function App() {
  return (
    <UpdateMatchingProvider>
      <AppInner />
    </UpdateMatchingProvider>
  );
}
