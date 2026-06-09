import { ArrowUp } from 'lucide-react';
import { Box, Flex } from 'styled-system/jsx';
import { IconButton, Textarea } from '@/ui/primitives';
import * as ScrollArea from '@/ui/primitives/scroll-area';

interface Message {
  id: number;
  sender: 'user' | 'assistant';
  text: string;
}

const mockMessages: Message[] = [
  { id: 1, sender: 'assistant', text: 'How can I help you today?' },
  { id: 2, sender: 'user', text: 'What does `git rebase` do?' },
  {
    id: 3,
    sender: 'assistant',
    text: "Git rebase moves the base of a branch to a new commit. It rewrites commit history, creating a linear project history. Unlike merge, it doesn't create a merge commit.",
  },
];

export function ChatPane() {
  return (
    <Box display="flex" flexDirection="column" h="full" pr="0.5">
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

      <Flex flexDirection="column" bg="gray.2" flex="1" borderRadius="l3">
        <ScrollArea.Root flex="1" size="lg">
          <ScrollArea.Viewport py="1" pr="3" pl="4">
            <ScrollArea.Content fontSize="sm">
              {mockMessages.map((msg) => (
                <Box
                  key={msg.id}
                  mb="3"
                  display="flex"
                  justifyContent={msg.sender === 'user' ? 'flex-end' : 'flex-start'}
                >
                  <Box
                    maxW="11/12"
                    px={msg.sender === 'user' ? '3' : undefined}
                    py="2"
                    borderRadius="l2"
                    borderWidth={msg.sender === 'user' ? '1' : '0'}
                    bg={msg.sender === 'user' ? 'gray.surface.bg' : undefined}
                  >
                    {msg.text}
                  </Box>
                </Box>
              ))}
            </ScrollArea.Content>
          </ScrollArea.Viewport>
        </ScrollArea.Root>

        <Flex p="2" position="relative">
          <Textarea
            size="sm"
            autoresize
            placeholder="Message the assistant"
            flex="1"
            maxH="40"
            pr="12"
          />
          <IconButton size="sm" borderRadius="full" position="absolute" right="4" bottom="4">
            <ArrowUp />
          </IconButton>
        </Flex>
      </Flex>
    </Box>
  );
}
