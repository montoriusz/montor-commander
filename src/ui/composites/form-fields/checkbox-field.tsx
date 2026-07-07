/**
 * Checkbox wired to `react-hook-form` via `Controller`.
 */
import { type Control, Controller, type FieldPath, type FieldValues } from 'react-hook-form';
import { Checkbox, Field } from '@/ui/primitives';

// ---------------------------------------------------------------------------
// CheckboxField
// ---------------------------------------------------------------------------

interface CheckboxFieldProps<T extends FieldValues> {
  control: Control<T>;
  name: FieldPath<T>;
  label: string;
  helperText?: string;
  disabled?: boolean;
  /** Called with the new checked value after the form field is updated. */
  onChange?: (checked: boolean) => void;
}

export function CheckboxField<T extends FieldValues>(props: Readonly<CheckboxFieldProps<T>>) {
  const { control, name, label, helperText, disabled, onChange } = props;
  return (
    <Controller
      control={control}
      name={name}
      render={({ field, fieldState }) => (
        <Field.Root invalid={fieldState.invalid}>
          <Checkbox.Root
            checked={Boolean(field.value)}
            onCheckedChange={(details) => {
              const checked = details.checked === true;
              field.onChange(checked);
              onChange?.(checked);
            }}
            disabled={disabled}
          >
            <Checkbox.HiddenInput />
            <Checkbox.Control>
              <Checkbox.Indicator />
            </Checkbox.Control>

            <Checkbox.Label>{label}</Checkbox.Label>
          </Checkbox.Root>
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
