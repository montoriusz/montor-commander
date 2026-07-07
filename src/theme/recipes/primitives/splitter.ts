import { splitterAnatomy } from '@ark-ui/react/anatomy';
import { defineSlotRecipe } from '@pandacss/dev';

export const splitter = defineSlotRecipe({
  className: 'splitter',
  slots: splitterAnatomy.keys(),
  base: {
    root: {
      display: 'flex',
      gap: '0.5',
    },
    panel: {
      borderRadius: 'l3',
      display: 'flex',
      background: 'gray.surface.bg',
      borderWidth: '1px',
      p: '2',
    },
    resizeTrigger: {
      transition: 'common',
      outline: '0',
      p: '1px',
      _horizontal: {
        minWidth: '5px',
        height: '95%',
        my: 'auto',
      },
      _vertical: {
        minHeight: '5px',
        width: '95%',
        mx: 'auto',
      },
      _hover: {
        '& [data-part="resize-trigger-indicator"]': {
          opacity: '0.75',
        },
      },
    },
    resizeTriggerIndicator: {
      opacity: '0',
      transition: 'opacity',
      background: 'gray.surface.fg',
      borderRadius: 'l3',
      m: 'auto',
      _horizontal: {
        width: '100%',
        height: '40%',
        maxHeight: '20rem',
      },
      _vertical: {
        height: '100%',
        width: '40%',
        maxWidth: '20rem',
      },
    },
  },
  variants: {
    variant: {
      full: {
        root: {
          gap: '1px',
          bg: 'surface',
          p: '1',
        },
        panel: {
          p: '0',
          background: 'transparent',
        },
      },
    },
  },
});
