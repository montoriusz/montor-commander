import { defineRecipe } from '@pandacss/dev';

export const prose = defineRecipe({
  className: 'prose',
  base: {
    paddingY: '2',

    lineHeight: '1.45',

    // Drop the trailing bottom margin of the last child so `.prose` blocks
    // sit flush with whatever follows them.
    '& > :first-child': { marginTop: '0' },
    '& > :last-child': { marginBottom: '0' },

    '& h1, & h2, & h3, & h4, & h5, & h6': {
      color: '{colors.fg.default}',
      fontWeight: '600',
      lineHeight: '1.3',
      marginTop: '1.5em',
      marginBottom: '0.5em',
    },
    '& h1': { fontSize: '1.75em' },
    '& h2': { fontSize: '1.5em' },
    '& h3': { fontSize: '1.25em' },
    '& h4, & h5, & h6': { fontSize: '1em' },

    '& p': { marginY: '0.5em' },

    '& a': {
      color: '{colors.blue.plain.fg}',
      textDecoration: 'none',
      _hover: { textDecoration: 'underline' },
    },

    '& strong': {
      color: '{colors.fg.default}',
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
      borderLeft: '3px solid {colors.blue.plain.fg}',
      paddingLeft: '1em',
      margin: '1em 0',
      color: '{colors.fg.subtle}',
      fontStyle: 'italic',
    },

    '& code': {
      fontSize: 'inherit',
    },

    '& hr': {
      border: 'none',
      borderTop: '1px solid {colors.border}',
      margin: '1.5em 0',
    },

    '& table': {
      width: '100%',
      borderCollapse: 'collapse',
      margin: '1em 0',
    },
    '& th, & td': {
      padding: '0.5em 0.75em',
      border: '1px solid {colors.gray.a6}',
    },
    '& th': {
      background: '{colors.gray.a2}',
      fontWeight: '600',
      textAlign: 'left',
    },
    '& td': { color: '{colors.fg.muted}' },
  },
});
