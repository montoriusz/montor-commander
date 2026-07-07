/**
 * Number input wired to `react-hook-form` via `Controller`.
 */
import { type Control, Controller, type FieldPath, type FieldValues } from 'react-hook-form';
import { Field, NumberInput } from '@/ui/primitives';

// ---------------------------------------------------------------------------
// NumberField
// ---------------------------------------------------------------------------

interface NumberFieldProps<T extends FieldValues> {
  control: Control<T>;
  name: FieldPath<T>;
  label: string;
  helperText?: string;
  min?: number;
  max?: number;
  step?: number;
  required?: boolean;
  width?: NumberInput.RootProps['width'];
  maxMidth?: NumberInput.RootProps['maxWidth'];
}

export function NumberField<T extends FieldValues>(props: Readonly<NumberFieldProps<T>>) {
  const { control, name, label, helperText, min, max, step, required, width } = props;
  return (
    <Controller
      control={control}
      name={name}
      render={({ field, fieldState }) => (
        <Field.Root invalid={fieldState.invalid} required={required}>
          <Field.Label>
            {label}
            <Field.RequiredIndicator />
          </Field.Label>
          <NumberInput.Root
            name={field.name}
            variant="surface"
            width={width}
            maxWidth={props.maxMidth}
            min={min}
            max={max}
            step={step}
            value={field.value == null ? '' : String(field.value)}
            onValueChange={(details) => {
              const next = details.value;
              field.onChange(next === '' ? null : Number(next));
            }}
            onBlur={field.onBlur}
          >
            <NumberInput.Control />
            <NumberInput.Input maxW="2xs" />
          </NumberInput.Root>
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
