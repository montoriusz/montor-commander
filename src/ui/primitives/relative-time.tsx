'use client';
import { Format } from '@ark-ui/react';
import { ark } from '@ark-ui/react/factory';
import { type ComponentProps, useEffect, useMemo, useRef, useState } from 'react';
import { styled } from 'styled-system/jsx';
import { relativeTime } from 'styled-system/recipes';

type TimeElementProps = ComponentProps<typeof BaseTime>;
const BaseTime = styled(ark.time, relativeTime);

interface RelativeTimeOptions {
  value: Date | string | number;
  /**
   * Automatically re-render the relative time as time passes.
   * The refresh cadence adapts to the age of the value.
   * @default true
   */
  autoRefresh?: boolean;
  /** Fixed refresh interval, overrides the adaptive cadence (ms). */
  refreshIntervalMs?: number;
  /** Custom formatter for the native `title` tooltip. */
  formatTitle?: (value: Date) => string;

  /** Options for the relative time formatter. */
  formatOptions?: Intl.RelativeTimeFormatOptions;
}

export interface RelativeTimeProps extends TimeElementProps, RelativeTimeOptions {}

const DEFAULT_FORMAT_OPTIONS: Intl.RelativeTimeFormatOptions = {
  numeric: 'auto',
};

const DEFAULT_TITLE_FORMATTER = new Intl.DateTimeFormat(undefined, {
  dateStyle: 'full',
  timeStyle: 'medium',
});

const DEFAULT_TITLE_FORMATTER_LOCALE = (value: Date) => DEFAULT_TITLE_FORMATTER.format(value);

/**
 * Returns the next refresh delay based on the age of the value.
 * Younger timestamps need finer granularity and refresh more often.
 */
function nextRefresh(ageMs: number): number {
  if (ageMs < 5_000) return 1_000; // < 5s  -> every second
  if (ageMs < 120_000) return 10_000; // < 2m  -> every 10s
  if (ageMs < 3_600_000) return 30_000; // < 1h  -> every 30s
  if (ageMs < 86_400_000) return 60_000; // < 1d  -> every minute
  return 300_000; // older -> every 5 min
}

export function RelativeTime(props: RelativeTimeProps) {
  const {
    value: valueProp,
    autoRefresh = true,
    refreshIntervalMs,
    formatTitle = DEFAULT_TITLE_FORMATTER_LOCALE,
    ...rest
  } = props;

  const date = new Date(valueProp);
  const timestampMs = date.getTime();
  const ageMs = Date.now() - timestampMs;
  // Re-rendered periodically to refresh the relative time display.
  const [, setTick] = useState(0);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const formatOptions = useMemo(
    () => ({ ...DEFAULT_FORMAT_OPTIONS, ...props.formatOptions }),
    [props.formatOptions],
  );

  useEffect(() => {
    if (!autoRefresh) return;

    const schedule = () => {
      const delay = refreshIntervalMs ?? nextRefresh(ageMs);
      timerRef.current = setTimeout(() => {
        setTick((t) => t + 1);
        schedule();
      }, delay);
    };

    schedule();

    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
    // `timestampMs` only changes when the underlying value does, ensuring the
    // effect re-initializes the timer if `valueProp` changes, while the
    // cadence adapts to age across tick-driven re-renders.
  }, [autoRefresh, refreshIntervalMs, ageMs]);

  return (
    <BaseTime dateTime={date.toISOString()} title={formatTitle(date)} {...rest}>
      <Format.RelativeTime {...formatOptions} value={date} />
    </BaseTime>
  );
}
