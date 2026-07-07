/**
 * Slider input wired to `react-hook-form` via `Controller`.
 */
import { type Control, Controller, type FieldPath, type FieldValues } from 'react-hook-form';
import { Flex } from 'styled-system/jsx';
import { Field, Slider } from '@/ui/primitives';

// ---------------------------------------------------------------------------
// SliderField
// ---------------------------------------------------------------------------

interface SliderFieldProps<T extends FieldValues> {
  control: Control<T>;
  name: FieldPath<T>;
  label: string;
  min: number;
  max: number;
  step?: number;
  /** Unit suffix rendered after the numeric value (e.g. `%`, `pt`). */
  unit?: string;
  helperText?: string;
  required?: boolean;
  width?: Slider.RootProps['width'];
}

export function SliderField<T extends FieldValues>(props: Readonly<SliderFieldProps<T>>) {
  const { control, name, label, min, max, step, unit, helperText, required, width } = props;
  const formatValue = (value: number | null | undefined) => {
    if (value == null) return '';
    return unit ? `${value}${unit}` : String(value);
  };
  return (
    <Controller
      control={control}
      name={name}
      render={({ field, fieldState }) => (
        <Field.Root invalid={fieldState.invalid} required={required}>
          <Slider.Root
            name={field.name}
            min={min}
            max={max}
            step={step}
            width={width}
            value={[field.value ?? min]}
            onValueChange={(details) => field.onChange(details.value[0])}
            onValueChangeEnd={(details) => field.onChange(details.value[0])}
            onBlur={field.onBlur}
            getAriaValueText={(details) => formatValue(details.value)}
          >
            <Flex justify="space-between">
              <Field.Label>
                {label}
                <Field.RequiredIndicator />
              </Field.Label>
              <Slider.ValueText>{formatValue(field.value)}</Slider.ValueText>
            </Flex>
            <Slider.Control>
              <Slider.Track>
                <Slider.Range />
              </Slider.Track>
              <Slider.Thumb index={0}>
                <Slider.HiddenInput />
              </Slider.Thumb>
            </Slider.Control>
          </Slider.Root>
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
