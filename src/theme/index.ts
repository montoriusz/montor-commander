import { animationStyles } from './animation-styles';
import { keyframes } from './keyframes';
import { layerStyles } from './layer-styles';
import { recipes, slotRecipes } from './recipes';
import { semanticTokens } from './semantic-tokens';
import { textStyles } from './text-styles';
import { colors } from './tokens/colors';
import { durations } from './tokens/durations';
import { fonts } from './tokens/fonts';
import { sizes } from './tokens/sizes';
import { zIndex } from './tokens/z-index';

export const themeExtend = {
  animationStyles,
  recipes,
  slotRecipes,
  keyframes,
  layerStyles,
  textStyles,

  tokens: {
    sizes,
    colors,
    durations,
    zIndex,
    fonts,
  },

  semanticTokens,
};

export { conditions } from './conditions';
export { globalCss } from './global-css';
