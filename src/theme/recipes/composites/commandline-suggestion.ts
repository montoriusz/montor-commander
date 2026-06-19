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
      maxWidth: '100%',
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
      p: '1',
      display: 'flex',
      alignItems: 'center',
      gap: '1',
      flexShrink: 0,
    },
  },
  defaultVariants: {
    variant: 'surface',
    size: 'md',
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
  },
});
