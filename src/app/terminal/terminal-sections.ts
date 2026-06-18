import type { IDecoration, IDisposable, ITerminalAddon, Terminal } from '@xterm/xterm';
import { token } from 'styled-system/tokens';

interface BufferMarker {
  x: number;
  y: number;
}

type MarkingPoint = 'PromptStart' | 'PromptEnd' | 'CommandStart' | 'CommandEnd';

interface Section {
  id: string;
  markers: Record<MarkingPoint, BufferMarker | undefined>;
  promptDecoration: IDecoration | undefined;
}

export interface SectionSnapshot {
  id: string;
  prompt: string;
  command?: string;
  output?: string;
  exitCode?: number;
}

export class TerminalSections implements ITerminalAddon {
  private terminal: Terminal | undefined;
  private oscHook: IDisposable | undefined;
  private sections: Section[] = [];
  private sectionsByAid: Map<string, Section> = new Map();
  private lastAid = '';

  activate(terminal: Terminal): void {
    this.terminal = terminal;
    console.log('register osc133 handler');

    const oscHandler = (data: string) => {
      const { marker, aid } = parseOsc133(data);

      if (marker === 'A') {
        console.log('PromptStart', aid);
        this.registerMarkingPoint('PromptStart', aid);
      } else if (marker === 'B') {
        this.registerMarkingPoint('PromptEnd', aid);
      } else if (marker === 'C') {
        this.registerMarkingPoint('CommandStart', aid);
      } else if (marker === 'D') {
        this.registerMarkingPoint('CommandEnd', aid);
      }
      return true;
    };

    this.oscHook = this.terminal.parser.registerOscHandler(133, oscHandler);
  }

  dispose() {
    this.oscHook?.dispose();
  }

  getLastSectionId(): string | undefined {
    return this.lastAid;
  }

  getLastExecutedSectionId(): string | undefined {
    const sectionIdx = this.sections.findLastIndex(
      (section) => section.markers.CommandStart !== undefined,
    );
    return sectionIdx !== -1 ? this.sections[sectionIdx - 1]?.id : undefined;
  }

  getSectionShapshots(afterAid: string | undefined): SectionSnapshot[] {
    if (!this.terminal) return [];

    const prevSection = afterAid !== undefined ? this.sectionsByAid.get(afterAid) : undefined;

    const startIdx = prevSection !== undefined ? this.sections.indexOf(prevSection) + 1 : 0;
    const sections = this.sections
      .slice(startIdx)
      .values()
      .filter((section) => section.markers.PromptStart !== undefined)
      .map<SectionSnapshot>((section) => {
        return {
          id: section.id,
          prompt: this.readFragment(section.markers.PromptStart!, section.markers.PromptEnd),
          command: section.markers.PromptEnd
            ? this.readFragment(section.markers.PromptEnd, section.markers.CommandStart)
            : undefined,
          output: section.markers.CommandStart
            ? this.readFragment(section.markers.CommandStart, section.markers.CommandEnd)
            : undefined,
        };
      })
      .toArray();

    return sections;
  }

  private orderSection(section: Section) {
    const sectionStartY = section.markers?.PromptStart?.y;
    if (sectionStartY === undefined) return;

    let currentIdx: number | undefined;
    let targetIdx: number | undefined;

    for (let i = 0; i < this.sections.length; i++) {
      if (this.sections[i].id === section.id) {
        currentIdx = i;
        continue;
      }

      const otherSectionStartY = this.sections[i].markers.PromptStart?.y;
      if (otherSectionStartY !== undefined && otherSectionStartY >= sectionStartY) {
        targetIdx = currentIdx === undefined ? i : i - 1;
        break;
      }
    }

    if (targetIdx === undefined) targetIdx = this.sections.length;

    if (currentIdx === targetIdx) return;

    if (currentIdx !== undefined) {
      this.sections.splice(currentIdx, 1);
    }
    this.sections.splice(targetIdx, 0, section);
  }

  private registerMarkingPoint(markingPoint: MarkingPoint, aid?: string) {
    if (!this.terminal) return;

    const effectiveAid = aid || this.lastAid;
    if (!effectiveAid) return;
    this.lastAid = effectiveAid;

    let section = this.sectionsByAid.get(effectiveAid);
    if (!section) {
      section = createSection(effectiveAid);
      this.sectionsByAid.set(effectiveAid, section);
    }

    section.markers[markingPoint] = {
      x: this.terminal.buffer.active.cursorX,
      y: this.terminal.buffer.active.cursorY + this.terminal.buffer.active.baseY,
    };

    this.orderSection(section);
    updateSectionDecorations(this.terminal, section);
  }

  private readFragment(start: BufferMarker, end?: BufferMarker): string {
    const buffer = this.terminal!.buffer.normal;
    const endY = end?.y ?? buffer.length - 1;
    const endX = end?.x;
    let result = '';
    for (let y = start.y; y <= endY; y++) {
      const line = buffer.getLine(y);
      const lineStr = line?.translateToString();
      if (!lineStr) continue;

      const begin = y === start.y ? start.x : 0;
      const end = y === endY && endX !== undefined ? endX : undefined;

      if (!line?.isWrapped) {
        result += '\n';
      }
      result += lineStr.slice(begin, end);
    }

    return result.trim();
  }
}

function createSection(id: string): Section {
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

const markingBorderColor = token('colors.green.surface.border');
const markingBgColor = '#121b17';
const markingRulerColor = '#20573e';

function updateSectionDecorations(term: Terminal, section: Section, force = false) {
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
