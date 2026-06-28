'use client';

import { Play, Terminal, X } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { css, cx } from 'styled-system/css';
import {
  type CommandlineSuggestionVariantProps,
  commandlineSuggestion,
} from 'styled-system/recipes';
import { Collapsible, IconButton } from '../primitives';

export type CommandlineSuggestionAction = 'execute' | 'put' | 'reject';

export interface CommandlineSuggestionProps extends CommandlineSuggestionVariantProps {
  commandline: string;
  suggestionId?: string;
  status?: 'pending' | 'accepted' | 'failed' | 'rejected';
  onAction?: (event: CommandlineSuggestionAction) => void;
}

const COLLAPSED_HEIGHT = '2.4em';

export function CommandlineSuggestion({
  status,
  commandline,
  suggestionId,
  onAction,
  ...props
}: CommandlineSuggestionProps) {
  const rootRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(status === 'pending');
  const [hasMore, setHasMore] = useState(false);

  useEffect(() => {
    if (status === 'pending') setOpen(true);
  }, [status]);

  useEffect(() => {
    const root = rootRef.current;
    const content = contentRef.current;
    if (!root || !content) return;
    const measure = () => {
      // The collapsible root clips to `collapsedHeight` when collapsed, so when
      // it is shorter than the full content there is hidden content below.
      setHasMore(content.scrollHeight > root.clientHeight + 1);
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(content);
    observer.observe(root);
    return () => observer.disconnect();
  }, []);

  const styles = commandlineSuggestion({
    ...props,
    hasMore: !open && hasMore,
  });
  let statusStyle = css({ colorPalette: 'blue' });
  if (status === 'pending') statusStyle = css({ colorPalette: 'amber' });
  else if (status === 'accepted') statusStyle = css({ colorPalette: 'green' });
  else if (status === 'failed') statusStyle = css({ colorPalette: 'red' });
  else if (status === 'rejected') statusStyle = css({ colorPalette: 'gray' });

  const handlers = useMemo(
    () => ({
      onApply: () => onAction?.('execute'),
      onPut: () => onAction?.('put'),
      onReject: () => onAction?.('reject'),
    }),
    [onAction],
  );

  return (
    <div className={cx(styles.root, statusStyle)} data-suggestion-id={suggestionId}>
      <Collapsible.Root
        ref={rootRef}
        variant="command"
        collapsedHeight={COLLAPSED_HEIGHT}
        className={styles.command}
        open={open}
        onOpenChange={(details) => setOpen(details.open)}
      >
        {/* TODO: highlight syntax */}
        <Collapsible.Content ref={contentRef}>{commandline}</Collapsible.Content>
        <Collapsible.Trigger
          disabled={!open && !hasMore}
          className={css({ position: 'absolute', inset: '0' })}
          // aria-label={open ? 'Collapse' : 'Expand'}
        />
        <Collapsible.Indicator />
      </Collapsible.Root>
      <div className={styles.actions}>
        <IconButton
          title="Execute"
          colorPalette={status === 'pending' ? 'green' : undefined}
          variant="subtle"
          size="xs"
          onClick={handlers.onApply}
        >
          <Play />
        </IconButton>
        {status === 'pending' ? (
          <IconButton
            title="Reject"
            colorPalette={'red'}
            variant="subtle"
            size="xs"
            onClick={handlers.onReject}
          >
            <X />
          </IconButton>
        ) : (
          <IconButton title="Send to terminal" variant="subtle" size="xs" onClick={handlers.onPut}>
            <Terminal />
          </IconButton>
        )}
      </div>
    </div>
  );
}
