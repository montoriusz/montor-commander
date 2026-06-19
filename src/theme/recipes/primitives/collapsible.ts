import { collapsibleAnatomy } from '@ark-ui/react/anatomy';
import { defineSlotRecipe } from '@pandacss/dev';

export const collapsible = defineSlotRecipe({
  className: 'collapsible',
  slots: collapsibleAnatomy.keys(),
  base: {
    content: {
      overflow: 'hidden',
      _open: {
        // TODO: make fade optional
        animationName: 'expand-height, fade-in',
        animationDuration: 'slow',
      },
      _closed: {
        animationName: 'collapse-height, fade-out',
        animationDuration: 'normal',
      },
    },
  },
  variants: {
    variant: {
      command: {
        content: {
          _open: {
            animationName: 'expand-max-height',
            animationDuration: 'slow',
          },
          _closed: {
            animationName: 'collapse-max-height',
            animationDuration: 'normal',
          },
        },
      },
    },
  },
});
