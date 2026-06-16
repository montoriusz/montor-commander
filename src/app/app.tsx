import { useRef } from 'react';
import {
  ResizeTriggerIndicator,
  Panel as SplitterPanel,
  ResizeTrigger as SplitterResizeTrigger,
  Root as SplitterRoot,
} from '@/ui/primitives/splitter';
import { ChatPane } from './chat';
import { TerminalPane, type TerminalHandle } from './terminal-pane';

export function App() {
  const terminalRef = useRef<TerminalHandle>(null);

  return (
    <SplitterRoot
      panels={[{ id: 'terminal' }, { id: 'chat' }]}
      variant="full"
      defaultSize={[5, 4]}
      orientation="horizontal"
      h="screen"
      w="screen"
      bg="border"
      p="[3px]"
      onResizeEnd={() => terminalRef.current?.fit()}
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
    </SplitterRoot>
  );
}
