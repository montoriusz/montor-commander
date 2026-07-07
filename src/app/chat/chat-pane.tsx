import { ArrowUp, Settings } from 'lucide-react';
import { Fragment, useCallback, useEffect, useRef, useState } from 'react';
import { css } from 'styled-system/css';
import { Box, Flex, styled, VStack } from 'styled-system/jsx';
import { sectionConnector } from 'styled-system/recipes';
import type { ChatMessage } from '@/generated';
import {
  CommandlineSuggestion,
  type CommandlineSuggestionAction,
} from '@/ui/composites/commandline-suggestion';
import { Markdown } from '@/ui/composites/markdown';
import { IconButton, RelativeTime, SkeletonText, Spinner, Textarea } from '@/ui/primitives';
import * as ScrollArea from '@/ui/primitives/scroll-area';
import { commandlineController, terminal, terminalSections } from '../terminal';
import { useEmitUpdateMatching } from '../terminal/section-matching';
import { windowManager } from '../window-manager';
import { useChat } from './use-chat';

interface MessageBubbleProps {
  msg: ChatMessage;
  isCurrentSection: boolean;
  onSuggestionAction: (event: CommandlineSuggestionAction, commandline: string) => void;
}

const RESIZE_DEBOUNCE_TIME = 300;

function MessageBubble({ msg, isCurrentSection, onSuggestionAction }: MessageBubbleProps) {
  const isUser = msg.type === 'User';
  // `cmdline` exists only on User/Assistant; null for TerminalSection so the
  // suggestion action is a no-op for those (which render nothing anyway).
  const cmdline = msg.type === 'Assistant' ? msg.cmdline : null;
  const actionHandler = useCallback(
    (event: CommandlineSuggestionAction) => {
      if (!cmdline) return;
      onSuggestionAction(event, cmdline);
    },
    [cmdline, onSuggestionAction],
  );

  // TerminalSection messages carry buffer content for re-render into a separate
  // xterm (Phase 3); they are not chat bubbles and render nothing here.
  if (msg.type === 'TerminalSection') return null;

  return (
    <Flex my="2" direction="column" alignItems={isUser ? 'end' : 'start'} minW="0" maxW="full">
      {msg.type === 'User' ? (
        <Flex
          maxW="11/12"
          px="3r"
          borderRadius="l3"
          borderWidth="1"
          bg="gray.surface.bg"
          color="gray.surface.fg"
          gap="2"
        >
          <Markdown content={msg.msg} />
        </Flex>
      ) : (
        <Flex w="11/12" flexDirection="column" gap="2">
          <Markdown content={msg.msg} />
          {!isUser && msg.cmdline && (
            <CommandlineSuggestion
              status={isCurrentSection ? 'pending' : undefined}
              suggestionId={msg.id}
              commandline={msg.cmdline}
              onAction={actionHandler}
            />
          )}
        </Flex>
      )}
      <Box color="fg.muted" fontSize="xs" mt="1">
        <RelativeTime value={msg.ts} />
        {msg.type === 'Assistant' ? <>&ensp;&bull;&ensp;{msg.model}</> : null}
      </Box>
    </Flex>
  );
}

// TODO: allow sending empty messages if terminal output is present

export function ChatPane() {
  const emitUpdateMatchingRef = useRef<() => void>(null);
  emitUpdateMatchingRef.current = useEmitUpdateMatching();
  const { messages, isGenerating, error, send } = useChat();
  const [input, setInput] = useState('');

  const viewportRef = useRef<HTMLDivElement | null>(null);
  // Last measured distance from the bottom, used by the ResizeObserver to keep
  // the user's scroll position stable when content reflows.
  const distanceFromBottomRef = useRef(0);

  // Preserve the distance from the bottom across viewport/content size changes
  // (e.g. markdown/images reflowing, window resize). Pinned-to-bottom stays
  // stuck to the bottom; any other position keeps the same offset.
  useEffect(() => {
    const el = viewportRef.current;
    if (!el) return;

    let isResizing = false;
    let isResizingTimeout: ReturnType<typeof setTimeout> | null = null;

    const onScroll = () => {
      if (isResizing) return;
      emitUpdateMatchingRef.current?.();
      distanceFromBottomRef.current = el.scrollHeight - el.scrollTop - el.clientHeight;
    };

    let observer: ResizeObserver | undefined;
    if (typeof ResizeObserver !== 'undefined') {
      observer = new ResizeObserver(() => {
        if (isResizingTimeout) clearTimeout(isResizingTimeout);
        isResizing = true;
        isResizingTimeout = setTimeout(() => {
          isResizing = false;
          isResizingTimeout = null;
        }, RESIZE_DEBOUNCE_TIME);
        el.scrollTop = Math.max(
          0,
          el.scrollHeight - el.clientHeight - distanceFromBottomRef.current,
        );
      });
      observer.observe(el);
    }

    el.addEventListener('scroll', onScroll);

    return () => {
      el.removeEventListener('scroll', onScroll);
      observer?.disconnect();
    };
  }, []);

  useEffect(() => {
    // Fake `isGenerating` usage
    isGenerating;
    if (messages.length === 0) return;

    emitUpdateMatchingRef.current?.();

    const el = viewportRef.current;
    if (!el || distanceFromBottomRef.current > 20) return;
    el.scrollTop = el.scrollHeight;
  }, [messages, isGenerating]);

  const handleSubmit = useCallback(() => {
    const text = input.trim();
    if (!text || isGenerating) return;
    setInput('');
    void send(text);
  }, [input, isGenerating, send]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        handleSubmit();
      }
    },
    [handleSubmit],
  );

  const suggestionActionHandler = useCallback(
    (event: CommandlineSuggestionAction, command: string) => {
      if (event === 'reject') {
        const lastUserCommandline = 'todo';
        commandlineController.put(lastUserCommandline);
      } else if (!command) {
        return;
      } else if (event === 'execute') {
        commandlineController.putAndExecute(command);
      } else if (event === 'put') {
        commandlineController.put(command);
        terminal.focus();
      }
    },
    [],
  );

  let lastTerminalStartSectionId: string | null = null;
  let lastTerminalEndSectionId: string | null = null;
  const getTerminalSectionBounds = (message: ChatMessage) => {
    if (message.type === 'TerminalSection' && message.exit_code != null) {
      if (lastTerminalStartSectionId === null) {
        lastTerminalStartSectionId = message.aid;
      }
      lastTerminalEndSectionId = message.aid;
      return undefined;
    } else if (lastTerminalStartSectionId !== null) {
      const result = [lastTerminalStartSectionId, lastTerminalEndSectionId];
      [lastTerminalStartSectionId, lastTerminalEndSectionId] = [null, null];
      return result;
    }
  };

  return (
    <Flex flexDirection="column" h="full" pr="1" flexGrow="1" overflow="hidden">
      <Flex bg="surface" alignItems="center">
        <Box p="3" fontWeight="semibold" textStyle="lg" flexGrow="1">
          Terminal Assistant
        </Box>
        <Box pr="2">
          <IconButton size="xs" variant="plain" onClick={openSettings}>
            <Settings />
          </IconButton>
        </Box>
      </Flex>

      <Flex
        flexDirection="column"
        bg="gray.2"
        flex="1"
        borderRadius="l3"
        borderWidth="1px"
        overflow="hidden"
      >
        <ScrollArea.Root flex="1" size="lg">
          <ScrollArea.Viewport ref={viewportRef} py="1" pr="3" pl="4">
            {messages.map((msg) => {
              const sectionBoundary = getTerminalSectionBounds(msg);
              return (
                <Fragment key={msg.id}>
                  {sectionBoundary && (
                    <styled.button
                      type="button"
                      onClick={connectorClickHandler}
                      className={sectionConnector({
                        separator: true,
                      })}
                      disabled={!isSectionConnectorActive(sectionBoundary[1])}
                      data-term-sect-id={sectionBoundary[1]}
                    />
                  )}
                  <MessageBubble
                    key={msg.id}
                    msg={msg}
                    onSuggestionAction={suggestionActionHandler}
                    isCurrentSection={false}
                  />
                </Fragment>
              );
            })}
            {isGenerating && (
              <VStack gap="2" my="3" alignItems="start">
                <div className={css({ color: 'fg.muted' })}>
                  <Spinner mr="2" size="xs" /> Thinking…
                </div>
                <SkeletonText />
              </VStack>
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

const connectorClickHandler = (e: React.MouseEvent) => {
  const target = e.target as HTMLElement;
  const sectionId = target.getAttribute('data-term-sect-id');
  if (sectionId) {
    terminalSections.scrollToSectionEnd(sectionId);
  }
};

const isSectionConnectorActive = (sectionId: string | null) => {
  if (sectionId == null) return false;
  return terminalSections.isSectionAvailable(sectionId);
};

const openSettings = () => {
  windowManager.open('settings');
};
