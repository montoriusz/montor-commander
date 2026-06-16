import { defineRecipe } from '@pandacss/dev';

export const prose = defineRecipe({
  className: 'prose',
  base: {
    lineHeight: '1.45',
    color: '{colors.text.secondary}',

    '& h1, & h2, & h3, & h4, & h5, & h6': {
      color: '{colors.text.primary}',
      fontWeight: '600',
      lineHeight: '1.3',
      marginTop: '1.5em',
      marginBottom: '0.5em',
    },
    '& h1': { fontSize: '1.75em' },
    '& h2': { fontSize: '1.5em' },
    '& h3': { fontSize: '1.25em' },
    '& h4, & h5, & h6': { fontSize: '1em' },

    '& p': { marginBottom: '0.75em' },

    '& a': {
      color: '{colors.accent.primary}',
      textDecoration: 'none',
      _hover: { textDecoration: 'underline' },
    },

    '& strong': {
      color: '{colors.text.primary}',
      fontWeight: '600',
    },

    '& em': { fontStyle: 'italic' },

    '& ul, & ol': {
      marginTop: '0.5em',
      marginBottom: '0.75em',
      paddingLeft: '1.5em',
    },
    '& li': { marginBottom: '0.25em' },
    '& ul': { listStyleType: 'disc' },
    '& ol': { listStyleType: 'decimal' },

    '& blockquote': {
      borderLeft: '3px solid {colors.accent.primary}',
      paddingLeft: '1em',
      margin: '1em 0',
      color: '{colors.text.muted}',
      fontStyle: 'italic',
    },

    '& code': {
      fontSize: 'inherit',
    },

    '& hr': {
      border: 'none',
      borderTop: '1px solid {colors.border.glass}',
      margin: '1.5em 0',
    },

    '& table': {
      width: '100%',
      borderCollapse: 'collapse',
      margin: '1em 0',
    },
    '& th, & td': {
      padding: '0.5em 0.75em',
      border: '1px solid {colors.border.glass}',
    },
    '& th': {
      background: '{colors.bg.glass}',
      fontWeight: '600',
      textAlign: 'left',
    },
    '& td': { color: '{colors.text.secondary}' },
  },
});
