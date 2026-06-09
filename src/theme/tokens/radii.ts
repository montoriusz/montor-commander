import { defineSemanticTokens } from '@pandacss/dev';

export const radii = defineSemanticTokens.radii({
  sm: {
    value: '6px',
  },
  md: {
    value: '12px',
  },
  lg: {
    value: '20px',
  },
  full: {
    value: '9999px',
  },
});
