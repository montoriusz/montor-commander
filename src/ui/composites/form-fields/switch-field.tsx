/**
 * Toggle switch wired to `react-hook-form` via `Controller`.
 */
import { type Control, Controller, type FieldPath, type FieldValues } from 'react-hook-form';
import { VisuallyHidden } from 'styled-system/jsx';
import { Field, Switch } from '@/ui/primitives';

// ---------------------------------------------------------------------------
// SwitchField
// ---------------------------------------------------------------------------

interface SwitchFieldProps<T extends FieldValues> {
  control: Control<T>;
  name: FieldPath<T>;
  label: string;
  helperText?: string;
  hiddenLabel?: boolean;
}

export function SwitchField<T extends FieldValues>(props: Readonly<SwitchFieldProps<T>>) {
  const { control, name, label, helperText, hiddenLabel = false } = props;
  return (
    <Controller
      control={control}
      name={name}
      render={({ field, fieldState }) => (
        <Field.Root invalid={fieldState.invalid}>
          <Switch.Root
            checked={Boolean(field.value)}
            onCheckedChange={(details) => field.onChange(details.checked)}
          >
            <Switch.HiddenInput />
            <Switch.Control />
          </Switch.Root>
          {hiddenLabel ? (
            <VisuallyHidden>
              <Field.Label>{label}</Field.Label>
            </VisuallyHidden>
          ) : (
            <Field.Label>{label}</Field.Label>
          )}
          {fieldState.error ? (
            <Field.ErrorText>{fieldState.error.message}</Field.ErrorText>
          ) : helperText ? (
            <Field.HelperText>{helperText}</Field.HelperText>
          ) : null}
        </Field.Root>
      )}
    />
  );
}
