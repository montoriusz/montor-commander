import { useEffect, useRef } from 'react';
import { useSettingsSlice } from './settings-store';

export interface UiSettingsProviderProps {
  onLightModeChange?: (isDark: boolean) => void;
  children: React.ReactNode;
}

export const UiSettingsProvider = ({ children, onLightModeChange }: UiSettingsProviderProps) => {
  const [uiSettings] = useSettingsSlice('ui');

  const onLightModeChangeRef = useRef(onLightModeChange);
  onLightModeChangeRef.current = onLightModeChange;

  useEffect(() => {
    document.documentElement.style.fontSize = `${uiSettings.uiScale}%`;

    const setMode = async (isDark: boolean) => {
      const cl = document.documentElement.classList;
      if (isDark) {
        cl.remove('light');
        cl.add('dark');
      } else {
        cl.remove('dark');
        cl.add('light');
      }

      onLightModeChangeRef.current?.(isDark);
    };

    if (uiSettings.theme === 'system') {
      const mq = window.matchMedia('(prefers-color-scheme: dark)');
      setMode(mq.matches);
      const handler = (e: MediaQueryListEvent) => setMode(e.matches);
      mq.addEventListener('change', handler);
      return () => {
        mq.removeEventListener('change', handler);
      };
    } else {
      setMode(uiSettings.theme === 'dark');
    }
  }, [uiSettings]);

  return children;
};
