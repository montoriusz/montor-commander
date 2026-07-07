import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { UiSettingsProvider } from './shared/ui-settings-provider';

const queryClient = new QueryClient();

export function CommonProviders({ children }: React.PropsWithChildren) {
  return (
    <UiSettingsProvider>
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    </UiSettingsProvider>
  );
}
