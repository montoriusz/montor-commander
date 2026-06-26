import { defineRecipe } from '@pandacss/dev'

export const relativeTime = defineRecipe({
  className: 'relativeTime',
  base: {
    display: 'inline-flex',
    alignItems: 'center',
    fontVariantNumeric: 'tabular-nums',
    whiteSpace: 'nowrap',
    lineHeight: '1',
  },
  defaultVariants: {
    variant: 'muted',
    size: 'xs',
  },
  variants: {
    variant: {
      muted: { color: 'fg.muted' },
      subtle: { color: 'fg.subtle' },
      plain: { color: 'fg.default' },
    },
    size: {
      xs: { textStyle: 'xs' },
      sm: { textStyle: 'sm' },
      md: { textStyle: 'md' },
      lg: { textStyle: 'lg' },
    },
  },
})
