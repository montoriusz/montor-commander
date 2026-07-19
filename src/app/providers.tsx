import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { UiSettingsProvider } from './shared/ui-settings-provider';

const queryClient = new QueryClient();

const isMainWindow = WebviewWindow.getCurrent().label === 'main';

const onLightModeChange = isMainWindow
  ? async (isDark: boolean) => {
      const { terminal, getTerminalTheme } = await import('./terminal');
      terminal.options.theme = getTerminalTheme(isDark);
    }
  : undefined;

export function CommonProviders({ children }: React.PropsWithChildren) {
  return (
    <UiSettingsProvider onLightModeChange={onLightModeChange}>
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    </UiSettingsProvider>
  );
}
