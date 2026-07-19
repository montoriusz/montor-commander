import { defineSlotRecipe } from '@pandacss/dev';

/**
 * A "slotted" textarea: it looks like the `textarea` recipe, but the border,
 * background and focus ring live on a `root` wrapper that encloses both the
 * actual `input` (textarea) and a `footer` slot for extra controls, so the
 * controls sit on the same surface and inside the same border.
 */
export const textareaSlot = defineSlotRecipe({
  className: 'textarea-slot',
  slots: ['root', 'input', 'footer'],
  base: {
    root: {
      appearance: 'none',
      borderRadius: 'l2',
      display: 'flex',
      flexDirection: 'column',
      minWidth: '0',
      position: 'relative',
      transition: 'colors',
      transitionProperty: 'box-shadow, border-color',
      width: '100%',
      _disabled: {
        layerStyle: 'disabled',
      },
    },
    input: {
      appearance: 'none',
      bg: 'transparent',
      border: '0',
      color: 'inherit',
      minWidth: '0',
      outline: '0',
      resize: 'none',
      width: '100%',
    },
    footer: {
      alignItems: 'center',
      display: 'flex',
      gap: '2',
    },
  },
  defaultVariants: {
    size: 'md',
    variant: 'surface',
  },
  variants: {
    variant: {
      outline: {
        root: {
          borderWidth: '1px',
          borderColor: 'gray.outline.border',
          '&:has(textarea:focus-visible)': {
            '--focus-ring-color':
              '[var(--focus-ring-color-prop, var(--global-color-focus-ring, #005FCC))]',
            outlineOffset: '[0px]',
            outlineWidth: '[var(--focus-ring-width, 1px)]',
            outlineStyle: '[var(--focus-ring-style, solid)]',
            outlineColor: '[var(--focus-ring-color)]',
            borderColor: '[var(--focus-ring-color)]',
          },
          _invalid: {
            borderColor: 'error',
            focusRingColor: 'error',
          },
        },
      },
      surface: {
        root: {
          bg: 'gray.surface.bg',
          borderWidth: '1px',
          borderColor: 'gray.surface.border',
          '&:has(textarea:focus-visible)': {
            '--focus-ring-color':
              '[var(--focus-ring-color-prop, var(--global-color-focus-ring, #005FCC))]',
            outlineOffset: '[0px]',
            outlineWidth: '[var(--focus-ring-width, 1px)]',
            outlineStyle: '[var(--focus-ring-style, solid)]',
            outlineColor: '[var(--focus-ring-color)]',
            borderColor: '[var(--focus-ring-color)]',
          },
          _invalid: {
            borderColor: 'error',
            focusRingColor: 'error',
          },
        },
      },
      subtle: {
        root: {
          bg: 'gray.subtle.bg',
          borderWidth: '1px',
          borderColor: 'transparent',
          color: 'gray.subtle.fg',
          '&:has(textarea:focus-visible)': {
            '--focus-ring-color':
              '[var(--focus-ring-color-prop, var(--global-color-focus-ring, #005FCC))]',
            outlineOffset: '[0px]',
            outlineWidth: '[var(--focus-ring-width, 1px)]',
            outlineStyle: '[var(--focus-ring-style, solid)]',
            outlineColor: '[var(--focus-ring-color)]',
            borderColor: '[var(--focus-ring-color)]',
          },
          _invalid: {
            borderColor: 'error',
            focusRingColor: 'error',
          },
        },
      },
    },
    size: {
      xs: {
        root: { pl: '2', pr: '1', py: '5px' },
        input: { pt: '5px', mt: '-5px', pr: '1', textStyle: 'sm', scrollPaddingBottom: '5px' },
      },
      sm: {
        root: { pl: '2.5', pr: '1', py: '7px' },
        input: { pt: '7px', mt: '-7px', pr: '1.5', textStyle: 'sm', scrollPaddingBottom: '7px' },
      },
      md: {
        root: { pl: '3', pr: '1.5', py: '7px', gap: '0.5' },
        input: { pt: '7px', mt: '-7px', pr: '1.5', textStyle: 'md', scrollPaddingBottom: '7px' },
      },
      lg: {
        root: { pl: '3.5', pr: '1.5', py: '9px', gap: '1' },
        input: { pt: '9px', mt: '-9px', pr: '2', textStyle: 'md', scrollPaddingBottom: '9px' },
      },
      xl: {
        root: { pl: '4', pr: '2', py: '9px', gap: '1' },
        input: { pt: '9px', mt: '-9px', pr: '2', textStyle: 'lg', scrollPaddingBottom: '9px' },
      },
    },
  },
});
