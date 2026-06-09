import { defineSemanticTokens } from '@pandacss/dev';

export const blurs = defineSemanticTokens.blurs({
  sm: {
    value: '4px',
  },
  md: {
    value: '8px',
  },
  lg: {
    value: '12px',
  },
  xl: {
    value: '16px',
  },
});
