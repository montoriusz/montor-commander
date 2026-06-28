import { useEffect, useMemo, useRef } from 'react';
import { type DebouncedFn, debounce } from './debounce';

export function useDebouncedCallback<Args extends unknown[]>(
  fn: (...args: Args) => void,
  delayMs: number,
): DebouncedFn<Args> {
  const fnRef = useRef(fn);
  fnRef.current = fn;

  const debounced = useMemo(() => debounce<Args>(fnRef.current, delayMs), [delayMs]);

  useEffect(() => debounced.cancel, [debounced]);

  return debounced;
}
