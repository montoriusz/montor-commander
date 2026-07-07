import { defineRecipe } from '@pandacss/dev';

export const sectionConnector = defineRecipe({
  className: 'section-connector',
  base: {
    position: 'relative',
    borderTopStyle: 'dotted',
    borderTopWidth: '2px',
    borderTopColor: 'sectionConnector',
    _hover: {
      borderTopColor: 'sectionConnector.hover',
    },
    _disabled: {
      _hover: {
        borderTopColor: 'sectionConnector',
      },
    },
    _before: {
      content: '""',
      position: 'absolute',
      inset: '-8px 0',
    },
  },
  variants: {
    separator: {
      true: {
        my: '2',
      },
    },
  },
});
