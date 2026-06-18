import { forwardRef, useCallback, useEffect, useImperativeHandle, useRef } from 'react';
import { Box } from 'styled-system/jsx';

import '@xterm/xterm/css/xterm.css';
import { fitTerminal, terminal } from './terminal';

export interface TerminalHandle {
  fit: () => void;
}

export const TerminalPane = forwardRef<TerminalHandle>(function TerminalPane(_, handleRef) {
  const containerRef = useRef<HTMLDivElement>(null);
  const fitTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const scheduleFit = useCallback(() => {
    if (fitTimerRef.current != null) clearTimeout(fitTimerRef.current);
    fitTimerRef.current = setTimeout(fitTerminal, 100);
  }, []);

  useImperativeHandle(handleRef, () => ({ fit: scheduleFit }), [scheduleFit]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    terminal.open(container);

    fitTerminal();
    window.addEventListener('resize', scheduleFit);

    return () => {
      window.removeEventListener('resize', scheduleFit);
      if (fitTimerRef.current != null) clearTimeout(fitTimerRef.current);
    };
  }, [scheduleFit]);

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
