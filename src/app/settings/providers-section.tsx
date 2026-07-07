/**
 * Settings → LLM Providers section.
 *
 * `useFieldArray` over providers, with a nested `useFieldArray` for each
 * provider's models. The primary provider field is `baseUrl`; the primary
 * model fields are `maxTokens` and `maxOutputTokens`. The API key is a regular
 * form field: the backend redacts a stored key to a placeholder sentinel, so
 * leaving it untouched round-trips through Save without exposing the secret,
 * clearing it removes the stored key, and typing a new value replaces it.
 *
 * A provider's `kind` is fixed at creation: "Add provider" is a drop-down that
 * picks the kind, and the `Kind` select is then disabled on the entry. Each
 * kind can have at most one `isPrimary` provider; checking Primary on an entry
 * clears it on the other entries of the same kind, and a new entry is primary
 * iff it is the first of its kind.
 */
import { yupResolver } from '@hookform/resolvers/yup';
import { CloudDownloadIcon, PlusIcon, Trash2Icon, UserRoundCogIcon } from 'lucide-react';
import { useCallback, useMemo, useRef, useState } from 'react';
import { type Control, type UseFormReturn, useFieldArray, useForm } from 'react-hook-form';
import { Box, Flex, Grid, GridItem, HStack, styled, VStack } from 'styled-system/jsx';
import { formatInvokeError } from '@/app/shared/invoke-error.helpers';
import { useSettingsFormStatusSync } from '@/app/shared/settings-form-status-store';
import { useSettingsSlice } from '@/app/shared/settings-store';
import { newProviderId, type ProviderMeta } from '@/generated';
import type { SelectOption } from '@/ui/composites/form-fields';
import { CheckboxField, NumberField, SwitchField, TextField } from '@/ui/composites/form-fields';
import { Accordion, Button, Collapsible, IconButton, Menu } from '@/ui/primitives';
import { toaster } from '@/ui/primitives/toast';
import { sanitizeAlias } from './provider-alias.helpers';
import {
  type AdapterKind,
  adapterKindLabel,
  type ProvidersFormValues,
  providersSectionSchema,
} from './providers-section.schemas';
import { useProvidersMeta } from './use-providers-meta';

const EMPTY_MODEL = { name: '', isCustom: true, maxTokens: null, maxOutputTokens: null };

type ProvidersControl = Control<ProvidersFormValues>;

const apiKeyHelperText = (stored: boolean, env: boolean): string =>
  stored
    ? env
      ? `Stored in keychain — clear the field to remove it and use environment key.`
      : `Stored in keychain — clear the field to remove it.`
    : env
      ? 'Using environment key. Enter a value to override it.'
      : 'No key set.';

export function ProvidersSectionForm() {
  const { data: providersMeta } = useProvidersMeta();
  const [llmSettings, saveProviders] = useSettingsSlice('llmProviders');

  const formRef = useRef<HTMLFormElement>(null);

  const form = useForm<ProvidersFormValues>({
    resolver: yupResolver(providersSectionSchema),
    defaultValues: { providers: [] },
    values: llmSettings,
  });

  useSettingsFormStatusSync('providers', form.control);

  const providerArray = useFieldArray({
    control: form.control,
    name: 'providers',
    keyName: 'fieldId',
  });

  const [openItems, setOpenItems] = useState<string[]>([]);

  const setPrimaryExclusive = useCallback(
    (kind: AdapterKind, primaryIndex: number) => {
      const providers = form.getValues('providers');

      for (const [i, p] of providers.entries()) {
        if (p.kind !== kind) continue;
        form.setValue(`providers.${i}.isPrimary`, i === primaryIndex, {
          shouldDirty: true,
          shouldValidate: true,
        });
      }
    },
    [form],
  );

  const addProvider = useCallback(
    async (kind: AdapterKind) => {
      const id = await newProviderId();
      const providers = form.getValues('providers');
      // A new entry is primary iff it is the first of its kind currently in the form.
      const isPrimary = !providers.some((p) => p.kind === kind);
      providerArray.append({
        id,
        name: '',
        alias: '',
        kind,
        baseUrl: undefined,
        enabled: true,
        isPrimary,
        models: [],
      });
      setOpenItems((prev) => (prev.includes(id) ? prev : [...prev, id]));
      setTimeout(() => {
        const item = formRef.current?.querySelector(`[data-provider-id="${id}"]`);
        if (item) item.scrollIntoView({ behavior: 'smooth' });
      });
    },
    [form, providerArray],
  );

  const onSubmit = form.handleSubmit(async (values) => {
    try {
      // TODO: fill alias suggestions
      await saveProviders(values);
      toaster.create({ title: 'Providers saved', type: 'success' });
    } catch (e) {
      toaster.create({
        title: 'Failed to save providers',
        description: formatInvokeError(e),
        type: 'error',
      });
    }
  });

  const kindMap = useMemo(
    () => new Map<AdapterKind, ProviderMeta>(providersMeta?.map((p) => [p.kind, p])),
    [providersMeta],
  );

  const kindOptions: SelectOption<AdapterKind>[] = useMemo(() => {
    return (providersMeta ?? []).map((p) => ({ value: p.kind, label: adapterKindLabel(p.kind) }));
  }, [providersMeta]);

  return (
    <form onSubmit={onSubmit} ref={formRef}>
      <VStack gap="4" alignItems="stretch">
        {providerArray.fields.length === 0 ? (
          <Box>No providers configured yet.</Box>
        ) : (
          <Accordion.Root
            collapsible
            multiple
            variant="leveled"
            value={openItems}
            onValueChange={(details) => setOpenItems(details.value)}
          >
            {providerArray.fields.map((field, index) => (
              <ProviderAccordionItem
                key={field.fieldId}
                itemId={field.id}
                index={index}
                control={form.control}
                form={form}
                kindMap={kindMap}
                setPrimaryExclusive={setPrimaryExclusive}
              />
            ))}
          </Accordion.Root>
        )}
        <HStack>
          <Menu.Root>
            <Menu.Trigger asChild>
              <Button variant="outline">
                <PlusIcon />
                Add provider
                <Menu.Indicator />
              </Button>
            </Menu.Trigger>
            <Menu.Positioner>
              <Menu.Content>
                {kindOptions.map((option) => (
                  <Menu.Item
                    key={option.value}
                    value={option.value}
                    onSelect={() => void addProvider(option.value)}
                  >
                    <Menu.ItemText>{option.label}</Menu.ItemText>
                  </Menu.Item>
                ))}
              </Menu.Content>
            </Menu.Positioner>
          </Menu.Root>
          <Button type="submit" disabled={!form.formState.isDirty || form.formState.isSubmitting}>
            {form.formState.isSubmitting ? 'Saving…' : 'Save providers'}
          </Button>
        </HStack>
      </VStack>
    </form>
  );
}

interface ProviderAccordionItemProps {
  itemId: string;
  index: number;
  control: ProvidersControl;
  form: UseFormReturn<ProvidersFormValues>;
  kindMap: Map<AdapterKind, ProviderMeta>;
  setPrimaryExclusive: (kind: AdapterKind, primaryIndex: number) => void;
}

function ProviderAccordionItem(props: Readonly<ProviderAccordionItemProps>) {
  const { itemId, index, control, form, kindMap, setPrimaryExclusive } = props;
  const modelArray = useFieldArray({ control, name: `providers.${index}.models` });
  const [entryName, kind, apiKey, isPrimary] = form.watch([
    `providers.${index}.name`,
    `providers.${index}.kind`,
    `providers.${index}.apiKey`,
    `providers.${index}.isPrimary`,
  ]);

  const kindMeta = kindMap.get(kind);
  const supportsCustomModels = kindMeta?.supportsCustomModels ?? false;
  const hasEnvKey = kindMeta?.envKeyProvided ?? false;
  const hasStoredKey = apiKey != null;

  const kindLabel = adapterKindLabel(kind);
  const displayedName = (isPrimary ? kindLabel : entryName) || `Provider ${index + 1}`;
  const suggestedAlias = useMemo(() => entryName && sanitizeAlias(entryName), [entryName]);

  // Checking Primary clears the flag on every other provider of the same kind.
  // Unchecking is already applied to this field by the Controller; nothing else
  // to do, since at-most-one is enforced only on the way up.
  const onPrimaryChange = useCallback(
    (checked: boolean) => {
      if (checked) setPrimaryExclusive(kind, index);
    },
    [index, kind, setPrimaryExclusive],
  );

  const addModel = useCallback(() => modelArray.append({ ...EMPTY_MODEL }), [modelArray]);
  const removeProvider = useCallback(() => {
    const arr = form.getValues('providers');
    arr.splice(index, 1);
    // Re-trigger validation by replacing the field value.
    form.setValue('providers', arr, { shouldDirty: true, shouldValidate: true });
  }, [index, form]);
  const clearKey = useCallback(
    () =>
      form.setValue(`providers.${index}.apiKey`, null, { shouldDirty: true, shouldValidate: true }),
    [index, form],
  );

  return (
    <Accordion.Item value={itemId} position="relative" data-provider-id={itemId}>
      <Accordion.ItemTrigger>
        <Box ml="24" flex="1" textAlign="left">
          {displayedName}
          {!isPrimary && (
            <styled.span fontWeight="normal" color="fg.muted" ml="2">
              {' '}
              {kindLabel}
            </styled.span>
          )}
        </Box>
        <Accordion.ItemIndicator />
      </Accordion.ItemTrigger>
      <Flex
        position="absolute"
        left="0"
        top="0"
        display="flex"
        alignItems="center"
        justifyContent="center"
        width="20"
        height="14"
      >
        <SwitchField
          control={control}
          name={`providers.${index}.enabled`}
          label="Enabled"
          hiddenLabel
        />
      </Flex>
      <Accordion.ItemContent>
        <Grid gap="4" gridTemplateColumns={{ base: '1fr', lg: '1fr 1fr' }}>
          <GridItem colStart={1} colEnd={-1}>
            <Flex justifyContent="space-between" gap="4" flexWrap="wrap">
              <CheckboxField
                control={control}
                name={`providers.${index}.isPrimary`}
                label={`Default provider for ${kindLabel}`}
                onChange={onPrimaryChange}
              />

              <Button variant="plain" size="sm" onClick={removeProvider}>
                <Trash2Icon />
                Remove provider
              </Button>
            </Flex>
            <Collapsible.Root open={!isPrimary}>
              <Collapsible.Content p="2" m="-2">
                <Grid gap="4" gridTemplateColumns={{ base: '1fr', md: '1fr 1fr' }} pt="4">
                  <TextField
                    control={control}
                    name={`providers.${index}.name`}
                    label="Provider name"
                    required={!isPrimary}
                    disabled={isPrimary}
                    placeholder={isPrimary ? kindLabel : displayedName}
                  />
                  <TextField
                    control={control}
                    name={`providers.${index}.alias`}
                    label="Alias (for records)"
                    placeholder={suggestedAlias}
                  />
                </Grid>
              </Collapsible.Content>
            </Collapsible.Root>
          </GridItem>

          <GridItem>
            <TextField
              control={control}
              name={`providers.${index}.apiKey`}
              label="API key"
              type="password"
              placeholder={hasEnvKey && kindMeta?.envKey ? `$${kindMeta.envKey}` : ''}
              helperText={apiKeyHelperText(hasStoredKey, hasEnvKey)}
              endElement={
                <IconButton variant="plain" size="xs" onClick={clearKey}>
                  <Trash2Icon aria-label="Clear API key" />
                </IconButton>
              }
            />
          </GridItem>

          <GridItem>
            <TextField
              control={control}
              name={`providers.${index}.baseUrl`}
              label="Base URL"
              placeholder="https://api.example.com/v1/"
              helperText="Provide if not using the default URL"
            />
          </GridItem>

          {supportsCustomModels && (
            <GridItem colStart={1} colEnd={-1} pt="4" borderTopWidth="1px">
              <HStack justifyContent="space-between">
                <Box>Models</Box>
                <HStack>
                  <Button size="xs" variant="surface">
                    <CloudDownloadIcon />
                    Fetch models
                  </Button>
                  <Button size="xs" variant="plain" onClick={addModel}>
                    <PlusIcon />
                    Add custom model
                  </Button>
                </HStack>
              </HStack>
              <VStack gap="3" alignItems="stretch" mt="4">
                {modelArray.fields.map((modelField, modelIndex) => (
                  <ModelRow
                    key={modelField.id}
                    form={form}
                    control={control}
                    index={index}
                    modelIndex={modelIndex}
                    onRemove={() => modelArray.remove(modelIndex)}
                  />
                ))}
              </VStack>
            </GridItem>
          )}
        </Grid>
      </Accordion.ItemContent>
    </Accordion.Item>
  );
}

interface ModelRowProps {
  index: number;
  form: UseFormReturn<ProvidersFormValues>;
  modelIndex: number;
  control: ProvidersControl;
  onRemove: () => void;
}

function ModelRow(props: Readonly<ModelRowProps>) {
  const { index, modelIndex, control, onRemove, form } = props;
  const namePath = `providers.${index}.models.${modelIndex}.name` as const;
  const maxTokensPath = `providers.${index}.models.${modelIndex}.maxTokens` as const;
  const maxOutputTokensPath = `providers.${index}.models.${modelIndex}.maxOutputTokens` as const;

  const isCustom = form.watch(`providers.${index}.models.${modelIndex}.isCustom`) ?? false;

  return (
    <HStack gap="2" alignItems="flex-end">
      <Box flex="1">
        <TextField
          control={control}
          name={namePath}
          label="Model name"
          required={isCustom}
          readOnly={!isCustom}
          startElement={
            isCustom ? (
              <div title="User-added model">
                <UserRoundCogIcon />
              </div>
            ) : (
              <div title="API-provided model">
                <CloudDownloadIcon />
              </div>
            )
          }
        />
      </Box>
      {isCustom && (
        <NumberField
          control={control}
          name={maxTokensPath}
          label="Max tokens"
          min={1}
          step={1}
          width="[8em]"
        />
      )}
      {isCustom && (
        <NumberField
          control={control}
          name={maxOutputTokensPath}
          label="Max output tokens"
          min={1}
          step={1}
          width="[8em]"
        />
      )}
      <IconButton aria-label="Remove model" variant="plain" size="sm" onClick={onRemove}>
        <Trash2Icon />
      </IconButton>
    </HStack>
  );
}

export function ProvidersSection() {
  return <ProvidersSectionForm />;
}
