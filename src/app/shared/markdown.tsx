import ReactMarkdown, { type Components } from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { oneDark } from 'react-syntax-highlighter/dist/esm/styles/prism';
import { Code } from '@/ui/primitives';
import { css } from 'styled-system/css';
import { prose } from 'styled-system/recipes';
import { Box } from 'styled-system/jsx';

const components: Components = {
  code({
    node,
    className,
    children,
    ref: _ref,
    style: _style,
    translate: _translate,
    color: _color,
    ...props
  }) {
    const match = /language-(\w+)/.exec(className || '');
    const inline = 'inline' in props ? !!props.inline : false;
    return !inline && match ? (
      <SyntaxHighlighter style={oneDark} language={match[1]} PreTag="div" {...props}>
        {String(children).replace(/\n$/, '')}
      </SyntaxHighlighter>
    ) : (
      <Code className={className} {...props} block={true} variant="subtle" colorPalette="gray">
        {children}
      </Code>
    );
  },
  // Customize other elements (h1, p, a, etc.)
};

const remarkPlugins = [remarkGfm];

export function Markdown({ content }: { content: string }) {
  return (
    <Box className={prose()}>
      <ReactMarkdown remarkPlugins={remarkPlugins} components={components}>
        {content}
      </ReactMarkdown>
    </Box>
  );
}
