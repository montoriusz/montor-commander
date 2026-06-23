import { useEffect, useMemo, useRef } from 'react';
import { type DebouncedFn, debounce } from './debounce';

export function useDebouncedCallback<Args extends unknown[]>(
  fn: (...args: Args) => void,
  delayMs: number,
): DebouncedFn<Args> {
  const fnRef = useRef(fn);
  useEffect(() => {
    fnRef.current = fn;
  });

  const debounced = useMemo(
    () => debounce<Args>((...args) => fnRef.current(...args), delayMs),
    [delayMs],
  );

  useEffect(() => debounced.cancel, [debounced]);

  return debounced;
}
