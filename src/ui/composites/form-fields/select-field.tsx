/**
 * Select input wired to `react-hook-form` via `Controller`.
 */
import { createListCollection, type ListCollection } from '@ark-ui/react/select';
import { type Control, Controller, type FieldPath, type FieldValues } from 'react-hook-form';
import { Field, Select } from '@/ui/primitives';

// ---------------------------------------------------------------------------
// SelectOption
// ---------------------------------------------------------------------------

export interface SelectOption<V extends string> {
  value: V;
  label: string;
}

// ---------------------------------------------------------------------------
// SelectField
// ---------------------------------------------------------------------------

interface SelectFieldProps<T extends FieldValues, V extends string> {
  control: Control<T>;
  name: FieldPath<T>;
  label: string;
  options: ReadonlyArray<SelectOption<V>>;
  placeholder?: string;
  helperText?: string;
  required?: boolean;
  disabled?: boolean;
  width?: Select.RootProps<unknown>['width'];
  maxWidth?: Select.RootProps<unknown>['maxWidth'];
}

export function SelectField<T extends FieldValues, V extends string>(
  props: Readonly<SelectFieldProps<T, V>>,
) {
  const { control, name, label, options, placeholder, helperText, required, disabled } = props;
  const collection: ListCollection<SelectOption<V>> = createListCollection({
    items: options as SelectOption<V>[],
  });
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
          <Select.Root
            minWidth="2xs"
            collection={collection}
            name={field.name}
            value={field.value ? [field.value] : []}
            onValueChange={(details) => field.onChange(details.value[0] as V)}
            onInteractOutside={() => field.onBlur()}
            disabled={disabled}
            variant="surface"
            width={props.width}
            maxWidth={props.maxWidth}
          >
            <Select.HiddenSelect />
            <Select.Control>
              <Select.Trigger>
                <Select.ValueText placeholder={placeholder} />
                <Select.Indicator />
              </Select.Trigger>
            </Select.Control>
            <Select.Positioner>
              <Select.Content>
                {collection.items.map((item) => (
                  <Select.Item key={item.value} item={item}>
                    <Select.ItemText>{item.label}</Select.ItemText>
                    <Select.ItemIndicator />
                  </Select.Item>
                ))}
              </Select.Content>
            </Select.Positioner>
          </Select.Root>
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
