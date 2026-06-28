'use client';
import { ark } from '@ark-ui/react/factory';
import { type FormatDistanceToNowOptions, formatDistanceToNow } from 'date-fns';
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
  titleFormatOptions?: Intl.DateTimeFormatOptions;
  /**
   * Whether to use second-level granularity for short durations.
   * Affects values between ~5s and ~60s.
   * @default true
   */
  includeSeconds?: boolean;
  /** Extra options passed to `date-fns` `formatDistanceToNow`. */
  formatOptions?: Omit<FormatDistanceToNowOptions, 'addSuffix' | 'includeSeconds'>;
}

export interface RelativeTimeProps extends TimeElementProps, RelativeTimeOptions {}

/** Threshold below which the value is shown verbatim as "now". */
const NOW_THRESHOLD_MS = 5_000;

const DEFAULT_TITLE_FORMAT_OPTIONS: Intl.DateTimeFormatOptions = {
  dateStyle: 'full',
  timeStyle: 'medium',
};

/**
 * Returns the next refresh delay based on the age of the value.
 * Younger timestamps need finer granularity and refresh more often.
 */
function nextRefresh(ageMs: number): number {
  if (ageMs < NOW_THRESHOLD_MS) return 2_500; // < 5s  -> 2.5s.
  if (ageMs < 120_000) return 5_000; // < 2m  -> every 5s
  if (ageMs < 2_700_000) return 30_000; // < 45m  -> every 30s
  if (ageMs < 86_400_000) return 180_000; // < 1d  -> 3 minutes
  return 360_000; // older -> every 6 min
}

function formatRelative(
  date: Date,
  includeSeconds: boolean,
  options: RelativeTimeOptions['formatOptions'],
): string {
  const ageMs = Date.now() - date.getTime();
  if (ageMs < NOW_THRESHOLD_MS) return 'now';

  return formatDistanceToNow(date, {
    addSuffix: true,
    includeSeconds,
    ...options,
  });
}

export function RelativeTime(props: RelativeTimeProps) {
  const {
    value: valueProp,
    autoRefresh = true,
    refreshIntervalMs,
    titleFormatOptions = DEFAULT_TITLE_FORMAT_OPTIONS,
    includeSeconds = true,
    formatOptions,
    ...rest
  } = props;

  // Re-rendered periodically to refresh the relative time display.
  const [, setTick] = useState(0);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const { date, isoString, timestampMs } = useMemo(() => {
    const date = new Date(valueProp);
    return {
      date,
      isoString: date.toISOString(),
      timestampMs: date.getTime(),
    };
  }, [valueProp]);

  useEffect(() => {
    if (!autoRefresh) return;

    const schedule = () => {
      const ageMs = Date.now() - timestampMs;
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
  }, [autoRefresh, refreshIntervalMs, timestampMs]);

  const titleFormatter = useMemo(() => {
    return new Intl.DateTimeFormat(undefined, titleFormatOptions);
  }, [titleFormatOptions]);

  const label = formatRelative(date, includeSeconds, formatOptions);
  const title = titleFormatter.format(date);

  return (
    <BaseTime dateTime={isoString} title={title} {...rest}>
      {label}
    </BaseTime>
  );
}
