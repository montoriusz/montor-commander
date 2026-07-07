import { forwardRef, useEffect, useImperativeHandle, useRef } from 'react';
import { Box } from 'styled-system/jsx';

import '@xterm/xterm/css/xterm.css';
import { useDebouncedCallback } from '../shared/use-debounced-callback';
import { useEmitUpdateMatching } from './section-matching';
import { fitTerminal, terminal } from './terminal';

export interface TerminalHandle {
  fit: () => void;
}

export const TerminalPane = forwardRef<TerminalHandle>(function TerminalPane(_, handleRef) {
  const containerRef = useRef<HTMLDivElement>(null);
  const emitUpdateMatching = useEmitUpdateMatching();
  const scheduleFit = useDebouncedCallback(fitTerminal, 100);

  useImperativeHandle(handleRef, () => ({ fit: scheduleFit }), [scheduleFit]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container || terminal.element === container) return;

    terminal.open(container);

    const scrollHandler = terminal.onRender(() => {
      emitUpdateMatching();
    });

    fitTerminal();
    window.addEventListener('resize', scheduleFit);

    return () => {
      window.removeEventListener('resize', scheduleFit);
      scrollHandler.dispose();
    };
  }, [scheduleFit, emitUpdateMatching]);

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
