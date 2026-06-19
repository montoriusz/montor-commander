import { ArrowUp } from 'lucide-react';
import { useState } from 'react';
import { Box, Flex } from 'styled-system/jsx';
import type { ChatMessage } from '@/generated';
import { CommandlineSuggestion } from '@/ui/composites/commandline-suggestion';
import { IconButton, Textarea } from '@/ui/primitives';
import * as ScrollArea from '@/ui/primitives/scroll-area';
import { Markdown } from '../shared/markdown';
import { useChat } from './use-chat';

interface MessageBubbleProps {
  msg: ChatMessage;
  isCurrentSection: boolean;
}

function MessageBubble({ msg, isCurrentSection: isCurretSection }: MessageBubbleProps) {
  const isUser = msg.type === 'User';
  return (
    <Flex py="2.5" justifyContent={isUser ? 'flex-end' : 'flex-start'} minW="0" maxW="full">
      <Flex
        maxW="11/12"
        px={isUser ? '3r' : undefined}
        py={isUser ? '0.5r' : undefined}
        borderRadius="l2"
        borderWidth={isUser ? '1' : '0'}
        bg={isUser ? 'gray.surface.bg' : undefined}
        flexDirection="column"
        gap="2"
      >
        <Markdown content={msg.msg} />
        {!isUser && msg.commandline && (
          <CommandlineSuggestion
            status={isCurretSection ? 'pending' : undefined}
            commandline={msg.commandline}
          />
        )}
      </Flex>
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

  const currentSectionIdx = isGenerating ? -1 : messages.length - 1;

  return (
    <Flex flexDirection="column" h="full" pr="0.5" flexGrow="1" overflow="hidden">
      <Box
        p="3"
        borderBottomWidth="1px"
        borderColor="border"
        fontWeight="semibold"
        fontSize="lg"
        bg="gray.4"
      >
        Terminal Assistant
      </Box>

      <Flex flexDirection="column" bg="gray.2" flex="1" borderRadius="l3" overflow="hidden">
        <ScrollArea.Root flex="1" size="lg">
          <ScrollArea.Viewport py="1" pr="3" pl="4">
            {messages.map((msg, idx) => (
              <MessageBubble key={msg.id} msg={msg} isCurrentSection={idx === currentSectionIdx} />
            ))}
            {isGenerating && (
              <Box my="3" color="fg.muted">
                Thinking…
              </Box>
            )}
          </ScrollArea.Viewport>
        </ScrollArea.Root>

        {error && (
          <Box px="3" py="1" color="red.4">
            {error}
          </Box>
        )}

        <Flex p="2" position="relative">
          <Textarea
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
