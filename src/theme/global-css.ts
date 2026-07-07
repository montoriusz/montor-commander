export const globalCss = {
  extend: {
    '*': {
      '--global-color-border': 'colors.border',
      '--global-color-placeholder': 'colors.fg.subtle',
      '--global-color-selection': 'colors.colorPalette.subtle.bg',
      '--global-color-focus-ring': 'colors.colorPalette.solid.bg',
    },
    ':root': {
      // TODO: move to settings
      colorPalette: 'green',
      background: 'surface',
    },
    body: {
      color: 'fg.default',
    },
    'html, body, #root': {
      width: 'full',
      height: 'full',
    },
  },
};
