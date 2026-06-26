import { defineSlotRecipe } from '@pandacss/dev';

export const commandlineSuggestion = defineSlotRecipe({
  className: 'commandline-suggestion',
  slots: ['root', 'command', 'actions'],
  base: {
    root: {
      alignItems: 'start',
      borderRadius: 'l2',
      display: 'flex',
      gap: '2',
      overflow: 'hidden',
      height: 'auto',
      minWidth: 'max(10rem, 30%)',
      width: 'max-content',
      maxWidth: 'full',
      position: 'relative',
      _after: {
        content: '""',
        display: 'block',
        position: 'absolute',
        bottom: '0',
        left: '0',
        right: '0',
        zIndex: '1',
        pointerEvents: 'none',
        //'--status-base-color': '{colors.black.a7}', // {colors.colorPalette.surface.border}',
      },
    },
    command: {
      position: 'relative',
      fontVariantNumeric: 'tabular-nums',
      fontWeight: 'medium',
      fontFamily: 'code',
      flexGrow: 1,
      minWidth: 0,
      whiteSpace: 'pre-wrap',
      wordBreak: 'break-all',
      lineHeight: '1.2',
    },
    actions: {
      zIndex: '100',
      p: '1',
      display: 'flex',
      alignItems: 'center',
      gap: '1',
      flexShrink: 0,
    },
  },
  variants: {
    variant: {
      solid: {
        root: {
          bg: 'colorPalette.solid.bg',
          color: 'colorPalette.solid.fg',
        },
      },
      surface: {
        root: {
          bg: 'colorPalette.surface.bg',
          borderWidth: '1px',
          borderColor: 'colorPalette.surface.border',
          color: 'colorPalette.surface.fg',
        },
      },
      subtle: {
        root: {
          bg: 'colorPalette.subtle.bg',
          color: 'colorPalette.subtle.fg',
        },
      },
      outline: {
        root: {
          borderWidth: '1px',
          borderColor: 'colorPalette.outline.border',
          color: 'colorPalette.outline.fg',
        },
      },
      plain: {
        root: {
          color: 'colorPalette.plain.fg',
        },
      },
    },
    size: {
      xs: { command: { textStyle: 'xs', px: '1.5', pt: '1.5' } },
      sm: { command: { textStyle: 'sm', px: '1.5', pt: '1.5' } },
      md: { command: { textStyle: 'md', px: '2', pt: '2' } },
      lg: { command: { textStyle: 'lg', px: '2.5', pt: '2.5' } },
      xl: { command: { textStyle: 'xl', px: '3', pt: '3' } },
    },
    hasMore: {
      true: {
        root: {
          _after: {
            height: '10px',
            // Solid fill masked by two intersecting linear gradients so the
            // fade tapers along both the top and the right edge (rectangular
            // corner fade) instead of only fading upward.
            // backgroundColor: 'var(--status-base-color)',
            backgroundColor: '{colors.black.a7}',
            maskImage:
              'linear-gradient(to top, black, transparent), linear-gradient(to right, black, transparent)',
            maskComposite: 'intersect',
            WebkitMaskImage:
              'linear-gradient(to top, black, transparent), linear-gradient(to right, black, black 70%, transparent 80%)',
            WebkitMaskComposite: 'source-in',
          },
        },
      },
    },
  },
  staticCss: ['*'],
  defaultVariants: {
    variant: 'surface',
    size: 'md',
  },
});
