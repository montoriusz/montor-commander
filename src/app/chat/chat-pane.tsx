import { useState } from 'react';
import { ArrowUp } from 'lucide-react';
import { Box, Flex } from 'styled-system/jsx';
import { IconButton, Textarea } from '@/ui/primitives';
import * as ScrollArea from '@/ui/primitives/scroll-area';
import { useChat } from './use-chat';
import type { ChatMessage } from '@/generated';
import { Markdown } from '../shared/markdown';

function MessageBubble({ msg }: { msg: ChatMessage }) {
  const isUser = msg.type === 'User';
  return (
    <Flex my="3" justifyContent={isUser ? 'flex-end' : 'flex-start'} minW="0" maxW="full">
      <Box
        maxW="11/12"
        px={isUser ? '3' : undefined}
        py="1"
        borderRadius="l2"
        borderWidth={isUser ? '1' : '0'}
        bg={isUser ? 'gray.surface.bg' : undefined}
      >
        <Markdown content={msg.msg} />
      </Box>
    </Flex>
  );
}

export function ChatPane() {
  const { messages, isGenerating, error, send } = useChat();
  const [input, setInput] = useState('');

  function handleSubmit() {
    const text = input.trim();
    if (!text || isGenerating) return;
    setInput('');
    void send(text);
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  }

  return (
    <Flex flexDirection="column" h="full" pr="0.5" flexGrow="1" overflow="hidden">
      <Box
        p="3"
        borderBottomWidth="1px"
        borderColor="border"
        fontWeight="semibold"
        fontSize="sm"
        bg="gray.4"
      >
        Terminal Assistant
      </Box>

      <Flex flexDirection="column" bg="gray.2" flex="1" borderRadius="l3" overflow="hidden">
        <ScrollArea.Root flex="1" size="lg" fontSize="sm">
          <ScrollArea.Viewport py="1" pr="3" pl="4">
            {messages.map((msg) => (
              <MessageBubble key={msg.id} msg={msg} />
            ))}
            {isGenerating && (
              <Box my="3" color="fg.muted" fontSize="sm">
                Thinking…
              </Box>
            )}
          </ScrollArea.Viewport>
        </ScrollArea.Root>

        {error && (
          <Box px="3" py="1" fontSize="xs" color="red.4">
            {error}
          </Box>
        )}

        <Flex p="2" position="relative">
          <Textarea
            size="sm"
            autoresize
            placeholder="Message the assistant"
            flex="1"
            maxH="40"
            pr="12"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
          />
          <IconButton
            size="sm"
            borderRadius="full"
            position="absolute"
            right="4"
            bottom="4"
            disabled={isGenerating || !input.trim()}
            onClick={handleSubmit}
          >
            <ArrowUp />
          </IconButton>
        </Flex>
      </Flex>
    </Flex>
  );
}
