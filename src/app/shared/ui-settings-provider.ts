import { useEffect } from 'react';
import { useSettingsSlice } from './settings-store';

export const UiSettingsProvider = ({ children }: { children: React.ReactNode }) => {
  const [uiSettings] = useSettingsSlice('ui');

  useEffect(() => {
    document.documentElement.style.fontSize = `${uiSettings.uiScale}%`;

    const setMode = (isDark: boolean) => {
      const cl = document.documentElement.classList;
      if (isDark) {
        cl.remove('light');
        cl.add('dark');
      } else {
        cl.remove('dark');
        cl.add('light');
      }
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
