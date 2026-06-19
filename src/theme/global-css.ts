export const globalCss = {
  extend: {
    '*': {
      '--global-color-border': 'colors.border',
      '--global-color-placeholder': 'colors.fg.subtle',
      '--global-color-selection': 'colors.colorPalette.subtle.bg',
      '--global-color-focus-ring': 'colors.colorPalette.solid.bg',
    },
    ':root': {
      fontSize: '92%',
      colorPalette: 'green',
      background: 'canvas',
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
