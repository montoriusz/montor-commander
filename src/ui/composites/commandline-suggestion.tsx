'use client';

import { Play, X } from 'lucide-react';
import { useMemo } from 'react';
import { css, cx } from 'styled-system/css';
import {
  type CommandlineSuggestionVariantProps,
  commandlineSuggestion,
} from 'styled-system/recipes';
import { Collapsible, IconButton } from '../primitives';

export interface CommandlineSuggestionProps extends CommandlineSuggestionVariantProps {
  commandline: string;
  status?: 'pending' | 'accepted' | 'failed' | 'rejected';
  onAction?: (event: 'accept' | 'reject') => void;
}

export function CommandlineSuggestion({
  status,
  commandline,
  onAction,
  ...props
}: CommandlineSuggestionProps) {
  const styles = commandlineSuggestion(props);
  let statusStyle = css({ colorPalette: 'blue' });
  if (status === 'pending') statusStyle = css({ colorPalette: 'amber' });
  else if (status === 'accepted') statusStyle = css({ colorPalette: 'green' });
  else if (status === 'failed') statusStyle = css({ colorPalette: 'red' });
  else if (status === 'rejected') statusStyle = css({ colorPalette: 'gray' });

  const handlers = useMemo(
    () => ({
      onAccept: () => onAction?.('accept'),
      onReject: () => onAction?.('reject'),
    }),
    [onAction],
  );

  return (
    <div className={cx(styles.root, statusStyle)}>
      <Collapsible.Root variant="command" collapsedHeight="2.2em" className={styles.command}>
        <Collapsible.Content minHeight="0 !important">{commandline}</Collapsible.Content>
        <Collapsible.Trigger
          className={css({ position: 'absolute', inset: '0' })}
          // aria-label={open ? 'Collapse' : 'Expand'}
        />
        <Collapsible.Indicator />
      </Collapsible.Root>
      {status === 'pending' && (
        <div className={styles.actions}>
          <IconButton colorPalette="green" variant="subtle" size="xs" onClick={handlers.onAccept}>
            <Play />
          </IconButton>
          <IconButton colorPalette="red" variant="subtle" size="xs" onClick={handlers.onReject}>
            <X />
          </IconButton>
        </div>
      )}
    </div>
  );
}
