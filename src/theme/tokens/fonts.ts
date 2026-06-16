import { defineSemanticTokens } from '@pandacss/dev';

export const fonts = defineSemanticTokens.fonts({
  body: {
    value: 'Inter, ui-sans-serif, system-ui, sans-serif',
  },
  code: {
    value: '"JetBrains Mono", ui-monospace, monospace',
  },
});
