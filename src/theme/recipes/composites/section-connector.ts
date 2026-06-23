import { defineRecipe } from '@pandacss/dev';

export const sectionConnector = defineRecipe({
  className: 'section-connector',
  base: {
    borderTopStyle: 'dotted',
    borderTopWidth: '2px',
    borderTopColor: 'sectionConnector',
  },
  variants: {
    separator: {
      true: {
        my: '2',
      },
    },
  },
});
