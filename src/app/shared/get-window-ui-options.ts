import type { WindowOptions } from '@tauri-apps/api/window';

export function getWindowUiOptions(): WindowOptions {
  const isDark = document.documentElement.classList.contains('dark');
  return {
    backgroundColor: isDark ? '#292929' : '#e8e8e8',
  };
}
