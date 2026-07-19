'use client';

import { useEffect, useMemo } from 'react';
import {
  type ModelSelectGroup,
  ModelSelectMenu,
  type ModelSelectOption,
} from '@/ui/composites/model-select-menu';
import { useChat } from './use-chat';
import { useLlmModels } from './use-llm-models';

/**
 * The chat pane's model-selection menu.
 *
 * Wires the generic {@link ModelSelectMenu} to the available-models query and
 * the chat-store's selected-model state. The store seeds the selected model
 * from the last assistant turn on init (see `chat-store`); when the chat
 * history is empty, the selection is `undefined` until the models query loads,
 * at which point the first group's first model is chosen.
 */
export function ModelMenu() {
  const { data: groups, isLoading } = useLlmModels();
  const { selectedModel, setSelectedModel } = useChat();

  // Adapt the generated `LlmProviderModels[]` shape to the generic menu's
  // `{ label, options: { value, label }[] }` structure.
  const menuGroups = useMemo<ModelSelectGroup[]>(
    () =>
      (groups ?? []).map((g) => ({
        label: g.providerName,
        options: g.models.map<ModelSelectOption>((m) => ({ value: m.alias, label: m.displayName })),
      })),
    [groups],
  );

  // Bootstrap an empty chat history's selection to the first addressable
  // model once the models query resolves. A non-empty history already has the
  // selection seeded by the store.
  useEffect(() => {
    if (selectedModel !== undefined) return;
    const first = firstAlias(menuGroups);
    if (first !== undefined) setSelectedModel(first);
  }, [menuGroups, selectedModel, setSelectedModel]);

  return (
    <ModelSelectMenu
      groups={menuGroups}
      value={selectedModel}
      onSelect={setSelectedModel}
      isLoading={isLoading}
    />
  );
}

function firstAlias(groups: ModelSelectGroup[]): string | undefined {
  for (const g of groups) {
    for (const o of g.options) {
      return o.value;
    }
  }
  return undefined;
}
