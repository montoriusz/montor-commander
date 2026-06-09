'use client';
import { VisuallyHidden } from 'styled-system/jsx';
import { Span } from './span';

export interface DisplayValueProps<T> {
  /** The value to display */
  value?: T | null;
  /** Optional function to format the value before displaying */
  formatValue?: (value: NonNullable<T>) => string | null;
}

export const DisplayValue = <T,>(props: DisplayValueProps<T>) => {
  const { value, formatValue } = props;

  const formattedValue = isNotEmpty(value) ? (formatValue?.(value) ?? String(value)) : null;

  if (formattedValue) {
    return formattedValue;
  }

  return (
    <>
      <Span color="fg.subtle" aria-hidden>
        —
      </Span>
      <VisuallyHidden>No value available</VisuallyHidden>
    </>
  );
};

const isString = (value: unknown): value is string => typeof value === 'string';

const isNotEmpty = <T,>(value: T | null | undefined): value is NonNullable<T> => {
  if (value == null) return false;
  if (isString(value) || Array.isArray(value)) return value.length > 0;
  return true;
};
