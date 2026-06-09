import { defineConfig } from '@pandacss/dev';
import { conditions, globalCss, themeExtend } from '@/theme';
import { amber } from '@/theme/colors/amber';
import { blue } from '@/theme/colors/blue';
import { green } from '@/theme/colors/green';
import { red } from '@/theme/colors/red';
import { sage } from '@/theme/colors/sage';

const isStorybook = false; // process.env.STORYBOOK === 'true';

const include = ['./src/(ui|app|features)/**/*.{js,jsx,ts,tsx}'];

if (isStorybook) {
  include.push('./src/stories/**/*.{js,jsx,ts,tsx}');
}

export default defineConfig({
  jsxFramework: 'react',

  // Whether to use css reset
  preflight: true,

  // Where to look for your css declarations
  include,

  plugins: [
    {
      name: 'Remove Panda Preset Colors',
      hooks: {
        'preset:resolved': ({ utils, preset, name }) =>
          name === '@pandacss/preset-panda'
            ? utils.omit(preset, ['theme.tokens.colors', 'theme.semanticTokens.colors'])
            : preset,
      },
    },
  ],

  strictTokens: true,

  staticCss: {
    themes: ['dark'],
  },

  // The following fragment is meant to be updated by the Park UI CLI,
  // thus the `theme` object must be unfolded here up to `colors` property.
  // `amber`, `gray`, `red`, `green`, and `blue` are core colours used directly by some components.
  // Further colours can be added running: `npx @park-ui/cli add <color>`
  // See https://park-ui.com/docs/theming for more details.
  theme: {
    extend: {
      ...themeExtend,

      semanticTokens: {
        ...themeExtend.semanticTokens,

        colors: {
          ...themeExtend.semanticTokens.colors,

          amber,
          gray: sage,
          red,
          green,
          blue,
        },
      },
    },
  },
  conditions,
  globalCss,

  // The output directory for your css system
  outdir: 'styled-system',
});
