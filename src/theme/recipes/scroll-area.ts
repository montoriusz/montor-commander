import { defineSlotRecipe } from '@pandacss/dev';

export const scrollArea = defineSlotRecipe({
  className: 'scroll-area',
  slots: ['root', 'viewport', 'content', 'corner'],
  base: {
    root: {
      display: 'flex',
      flexDirection: 'column',
      width: '100%',
      height: '100%',
      position: 'relative',
      overflow: 'hidden',
    },
    viewport: {
      display: 'flex',
      flexDirection: 'column',
      height: '100%',
      width: '100%',
      WebkitOverflowScrolling: 'touch',
      scrollbarWidth: 'none',
      '&[data-overflow-x] [data-pinned]': {
        _after: {
          content: '""',
          position: 'absolute',
          pointerEvents: 'none',
          top: '0',
          bottom: '-1px',
          width: '32px',
        },
      },
      '&[data-overflow-x]:not([data-at-left]) [data-pinned="left"]': {
        _after: {
          insetInlineEnd: '0',
          translate: '100% 0',
          boxShadow: 'inset',
        },
      },
    },
    corner: {},
  },
  defaultVariants: {
    size: 'md',
    scrollbar: 'auto',
  },
  variants: {
    scrollbar: {
      auto: {
        viewport: {
          scrollbarWidth: 'auto',
        },
      },
      visible: {
        viewport: {
          scrollbarWidth: 'auto',
          scrollbarGutter: 'stable',
        },
      },
    },
    size: {
      xs: { viewport: { scrollbarWidth: 'thin' } },
      sm: { viewport: { scrollbarWidth: 'thin' } },
      md: {},
      lg: {},
    },
  },
});
