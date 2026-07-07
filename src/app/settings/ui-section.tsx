/**
 * Settings → UI section form.
 *
 * A self-contained `react-hook-form` instance driven by the [`useUiSettings`]
 * store hook. Save rounds the values to the backend; the backend's
 * `settings-updated` event refetches and reconciles the form via RHF's `values`
 * option.
 */
import { yupResolver } from '@hookform/resolvers/yup';
import { useForm } from 'react-hook-form';
import { Flex, Grid, GridItem, VStack } from 'styled-system/jsx';
import { formatInvokeError } from '@/app/shared/invoke-error.helpers';
import { useSettingsFormStatusSync } from '@/app/shared/settings-form-status-store';
import { useSettingsSlice } from '@/app/shared/settings-store';
import { SelectField, type SelectOption, SliderField } from '@/ui/composites/form-fields';
import { Button } from '@/ui/primitives';
import { toaster } from '@/ui/primitives/toast';
import { type UiFormValues, uiSettingsSchema } from './ui-section.schemas';

const THEME_OPTIONS: ReadonlyArray<SelectOption<UiFormValues['theme']>> = [
  { value: 'system', label: 'System' },
  { value: 'light', label: 'Light' },
  { value: 'dark', label: 'Dark' },
];

export function UiSectionForm() {
  const [ui, save] = useSettingsSlice('ui');

  const form = useForm<UiFormValues>({
    resolver: yupResolver(uiSettingsSchema),
    defaultValues: { theme: 'system', fontSize: 13, uiScale: 100 } satisfies UiFormValues,
    values: ui,
  });

  useSettingsFormStatusSync('ui', form.control);

  const onSubmit = form.handleSubmit(async (values) => {
    try {
      await save(values);
      toaster.create({ title: 'UI settings saved', type: 'success' });
    } catch (e) {
      toaster.create({
        title: 'Failed to save UI settings',
        description: formatInvokeError(e),
        type: 'error',
      });
    }
  });

  return (
    <form onSubmit={onSubmit}>
      <VStack gap="4" alignItems="stretch">
        <Flex gap="4" flexDirection="column">
          <SelectField
            control={form.control}
            name="theme"
            label="Theme"
            options={THEME_OPTIONS}
            required
            maxWidth="sm"
          />
          <SliderField
            control={form.control}
            name="uiScale"
            label="UI scale"
            min={50}
            max={200}
            step={1}
            unit="%"
            required
            width="full"
          />
        </Flex>
        <Button type="submit" disabled={!form.formState.isDirty || form.formState.isSubmitting}>
          {form.formState.isSubmitting ? 'Saving…' : 'Save UI settings'}
        </Button>
      </VStack>
    </form>
  );
}

export function UiSection() {
  return <UiSectionForm />;
}
