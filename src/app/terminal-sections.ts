import type { IDecoration, Terminal } from '@xterm/xterm';
import { token } from 'styled-system/tokens';

export interface BufferMarker {
  x: number;
  y: number;
}

export type MarkingPoint = 'PromptStart' | 'PromptEnd' | 'CommandStart' | 'CommandEnd';

export interface Section {
  id: string;
  markers: Record<MarkingPoint, BufferMarker | undefined>;
  promptDecoration: IDecoration | undefined;
}

export function createSection(id: string): Section {
  return {
    id,
    markers: {
      PromptStart: undefined,
      PromptEnd: undefined,
      CommandStart: undefined,
      CommandEnd: undefined,
    },
    promptDecoration: undefined,
  };
}

const markingBorderColor = token('colors.green.surface.border');
const markingBgColor = '#121b17';
const markingRulerColor = '#20573e';

export function updateSectionDecorations(term: Terminal, section: Section, force = false) {
  if (
    (!section.promptDecoration || force) &&
    section.markers.PromptStart &&
    section.markers.PromptEnd &&
    section.markers.CommandStart
  ) {
    if (section.promptDecoration) {
      section.promptDecoration.dispose();
      section.promptDecoration = undefined;
    }

    const marker = term.registerMarker(
      section.markers.PromptStart.y - term.buffer.active.cursorY - term.buffer.active.baseY,
    );
    if (marker) {
      const decoration = term.registerDecoration({
        marker,
        x: section.markers.PromptStart.x,
        width: term.cols - section.markers.PromptStart.x,
        layer: 'bottom',
        backgroundColor: markingBgColor,
        overviewRulerOptions: {
          color: markingRulerColor,
          position: 'full',
        },
      });
      if (decoration) {
        section.promptDecoration = decoration;
        decoration.onRender((element: HTMLElement) => {
          element.style.borderTop = `2px solid ${markingBorderColor}`;
          element.style.transform = `translateY(-1px)`;
        });
        decoration.onDispose(() => {
          decoration.marker.dispose();
          section.promptDecoration = decoration;
        });
      } else {
        marker.dispose();
      }
    }
  }
}

const AIP_PREFIX = 'aid=';
export function parseOsc133(data: string): {
  marker: string;
  exitCode: number | undefined;
  aid: string | undefined;
} {
  const parts = data.split(';');
  const marker = parts[0];
  let exitCode: number | undefined;
  let aid: string | undefined;

  for (const part of parts.slice(1)) {
    if (part.startsWith(AIP_PREFIX)) {
      aid = part.slice(AIP_PREFIX.length);
    } else if (marker === 'D' && exitCode === undefined) {
      exitCode = parseInt(part, 10);
    }
  }

  return { marker, exitCode, aid };
}
