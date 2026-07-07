/**
 * Text input wired to `react-hook-form` via `Controller`.
 */
import type { ComponentProps, ReactNode } from 'react';
import { type Control, Controller, type FieldPath, type FieldValues } from 'react-hook-form';
import { Field, Input, InputGroup } from '@/ui/primitives';

// ---------------------------------------------------------------------------
// TextField
// ---------------------------------------------------------------------------

interface TextFieldProps<T extends FieldValues> {
  control: Control<T>;
  name: FieldPath<T>;
  label: string;
  placeholder?: string;
  helperText?: string;
  type?: ComponentProps<typeof Input>['type'];
  required?: boolean;
  disabled?: boolean;
  readOnly?: boolean;
  startElement?: ReactNode;
  endElement?: ReactNode;
}

export function TextField<T extends FieldValues>(props: Readonly<TextFieldProps<T>>) {
  const {
    control,
    name,
    label,
    placeholder,
    helperText,
    type = 'text',
    required,
    disabled,
    readOnly,
    startElement,
    endElement,
  } = props;
  return (
    // TODO: make uncontrolled
    <Controller
      control={control}
      name={name}
      render={({ field, fieldState }) => {
        const input = (
          <Input
            {...field}
            value={field.value ?? ''}
            type={type}
            placeholder={placeholder}
            disabled={disabled}
            readOnly={readOnly}
            variant="surface"
          />
        );

        return (
          <Field.Root invalid={fieldState.invalid} required={required}>
            <Field.Label>
              {label}
              <Field.RequiredIndicator />
            </Field.Label>
            {startElement || endElement ? (
              <InputGroup startElement={startElement} endElement={endElement}>
                {input}
              </InputGroup>
            ) : (
              input
            )}
            {fieldState.error ? (
              <Field.ErrorText>{fieldState.error.message}</Field.ErrorText>
            ) : helperText ? (
              <Field.HelperText>{helperText}</Field.HelperText>
            ) : null}
          </Field.Root>
        );
      }}
    />
  );
}
