import { PencilIcon, TriangleAlertIcon } from 'lucide-react';
import { Suspense } from 'react';
import { Box, Flex, VStack } from 'styled-system/jsx';
import { type SettingsTabId, useSettingsFormStatus } from '@/app/shared/settings-form-status-store';
import { Heading, Icon, Tabs } from '@/ui/primitives';
import { ProvidersSection } from '../settings/providers-section';
import { UiSection } from '../settings/ui-section';

interface SettingsTab {
  id: SettingsTabId;
  label: string;
  content: React.ReactNode;
}

const TABS: ReadonlyArray<SettingsTab> = [
  {
    id: 'ui',
    label: 'UI',
    content: <UiSection />,
  },
  {
    id: 'providers',
    label: 'LLM Providers',
    content: <ProvidersSection />,
  },
];

function StatusMarker({ id }: { id: SettingsTabId }) {
  const { dirty, hasErrors } = useSettingsFormStatus(id);

  let icon: React.ElementType;
  let title: string;

  if (hasErrors) {
    icon = TriangleAlertIcon;
    title = 'Errors';
  } else if (dirty) {
    icon = PencilIcon;
    title = 'Unsaved changes';
  } else {
    return null;
  }

  return (
    <Box as="span" color="warning" title={title}>
      <Icon as={icon} size="sm" />
    </Box>
  );
}

export function SettingsWindow() {
  return (
    <Tabs.Root
      variant="enclosed"
      orientation="vertical"
      defaultValue="ui"
      h="screen"
      w="screen"
      bg="surface"
      p="2"
      gap="2"
    >
      <Tabs.List h="full" w="1/5" minW="2xs" maxW="sm">
        {TABS.map((tab) => (
          <Tabs.Trigger key={tab.id} value={tab.id}>
            <Flex w="full" gap="2" justifyContent="space-between" alignItems="center">
              {tab.label}
              <StatusMarker id={tab.id} />
            </Flex>
          </Tabs.Trigger>
        ))}
        <Tabs.Indicator />
      </Tabs.List>
      {TABS.map((tab) => (
        <Tabs.Content
          key={tab.id}
          value={tab.id}
          h="full"
          flex="1"
          overflow="auto"
          alignItems="start"
        >
          <Suspense
            fallback={
              <VStack alignItems="center" justify="center" h="full">
                <Box>Loading…</Box>
              </VStack>
            }
          >
            <Flex gap="8" direction="column" p="2">
              <Heading>{tab.label}</Heading>
              {tab.content}
            </Flex>
          </Suspense>
        </Tabs.Content>
      ))}
    </Tabs.Root>
  );
}
