import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import type { ReactElement } from 'react';
import { createRoot } from 'react-dom/client';
import type { SettingsCategory } from '@/generated';
import {
  ALL_SETTINGS_CATEGORIES,
  initSettingStore,
  type SettingsCategories,
} from './shared/settings-store';

export type WebviewWindowOptions = ConstructorParameters<typeof WebviewWindow>[1];

export interface WindowDefinition {
  label: string;
  rootElement(): Promise<ReactElement>;
  windowOptions?: WebviewWindowOptions | (() => Promise<WebviewWindowOptions>);
  observedSettings?: SettingsCategories;
}

export class WindowManagerBuilder {
  private readonly windowDefinitions = new Map<string, WindowDefinition>();
  private commonProvidersComponent: React.ComponentType<React.PropsWithChildren> | undefined;
  private defaultSettingsCategoriesList: SettingsCategory[] = [];
  private defaultWindoweOptionsFn: (() => WebviewWindowOptions) | undefined;

  register(...windows: WindowDefinition[]): this {
    for (const window of windows) {
      this.windowDefinitions.set(window.label, window);
    }
    return this;
  }

  commonProviders(providers: React.ComponentType<React.PropsWithChildren>): this {
    this.commonProvidersComponent = providers;
    return this;
  }

  defaultSettingsCategories(categories: SettingsCategory[]): this {
    this.defaultSettingsCategoriesList = categories;
    return this;
  }

  defaultWindowOptions(fn: () => WebviewWindowOptions): this {
    this.defaultWindoweOptionsFn = fn;
    return this;
  }

  async bootstrap(): Promise<void> {
    const currentWindow = WebviewWindow.getCurrent();
    const { label } = currentWindow;
    const windowDefinition = this.windowDefinitions.get(label);
    if (!windowDefinition) {
      throw new Error(`Window definition not found for label: ${label}`);
    }

    await this.renderWindow(windowDefinition);

    void currentWindow.show();
  }

  async open(label: string): Promise<WebviewWindow> {
    const existing = await WebviewWindow.getByLabel(label);
    if (existing) {
      await existing.setFocus();
      return existing;
    }

    const windowOptionsArg = this.windowDefinitions.get(label)?.windowOptions;

    if (!windowOptionsArg) {
      throw new Error(`Window definition not found for label: ${label}`);
    }

    let windowDefinition: WebviewWindowOptions = {};
    if (typeof windowOptionsArg === 'function') {
      windowDefinition = await windowOptionsArg();
    } else if (typeof windowOptionsArg === 'object') {
      windowDefinition = windowOptionsArg;
    }

    return new WebviewWindow(label, {
      visible: false, // Workaround for white flash
      ...(this.defaultWindoweOptionsFn?.() ?? {}),
      ...windowDefinition,
    });
  }

  private async renderWindow(windowDefinition: WindowDefinition): Promise<void> {
    const [_, rootJsx] = await Promise.all([
      this.initSettingStore(windowDefinition),
      this.initJsx(windowDefinition),
    ]);

    const rootEl = document.getElementById('root')!;
    createRoot(rootEl).render(rootJsx);
  }

  private async initJsx(windowDefinition: WindowDefinition): Promise<React.ReactNode> {
    let rootJsx: React.ReactNode = await windowDefinition.rootElement();
    const CommonProviders = this.commonProvidersComponent;
    if (CommonProviders) {
      rootJsx = <CommonProviders>{rootJsx}</CommonProviders>;
    }
    return rootJsx;
  }

  private async initSettingStore(windowDefinition: WindowDefinition): Promise<void> {
    await initSettingStore(
      addToSettingsCategory(windowDefinition.observedSettings, this.defaultSettingsCategoriesList),
    );
  }
}

export async function close(label: string): Promise<void> {
  const window = await WebviewWindow.getByLabel(label);
  window?.close();
}

export function closeCurrent(): void {
  const currentWindow = WebviewWindow.getCurrent();
  currentWindow?.close();
}

function addToSettingsCategory(
  source: SettingsCategories | undefined,
  newCategories: SettingsCategory[],
): SettingsCategories {
  if (source === ALL_SETTINGS_CATEGORIES) return ALL_SETTINGS_CATEGORIES;
  return Array.from(new Set<SettingsCategory>([...(source ?? []), ...newCategories]));
}
