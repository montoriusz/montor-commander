import { shadows } from './tokens/shadows';

export const semanticTokens = {
  colors: {
    canvas: {
      value: '#0a0a0a',
      // value: {
      //   _light: '{colors.white}',
      //   _dark: '{colors.black}',
      // },
    },

    fg: {
      default: {
        value: {
          _light: '{colors.gray.12}',
          _dark: '{colors.gray.12}',
        },
      },

      muted: {
        value: {
          _light: '{colors.gray.11}',
          _dark: '{colors.gray.11}',
        },
      },

      subtle: {
        value: {
          _light: '{colors.gray.10}',
          _dark: '{colors.gray.10}',
        },
      },
    },

    surface: {
      value: {
        _light: '{colors.gray.4}',
        _dark: '{colors.gray.4}',
      },
    },

    border: {
      value: {
        _light: '{colors.gray.6}',
        _dark: '{colors.gray.6}',
      },
    },

    success: {
      value: {
        _light: '{colors.green.9}',
        _dark: '{colors.green.9}',
      },
    },

    warning: {
      value: {
        _light: '{colors.amber.10}',
        _dark: '{colors.amber.9}',
      },
    },

    error: {
      value: {
        _light: '{colors.red.9}',
        _dark: '{colors.red.9}',
      },
    },

    sectionConnector: {
      DEFAULT: {
        value: {
          _light: '#80808080',
          _dark: '#80808080',
        },
      },
      hover: {
        value: {
          _light: '#808080',
          _dark: '#808080',
        },
      },
    },
  },

  shadows,

  radii: {
    l1: { value: '{radii.xs}' },
    l2: { value: '{radii.sm}' },
    l3: { value: '{radii.md}' },
  },
};
