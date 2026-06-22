import type { IDecoration, IDisposable, ITerminalAddon, Terminal } from '@xterm/xterm';

interface BufferMarker {
  x: number;
  y: number;
}

type MarkingPoint = 'PromptStart' | 'PromptEnd' | 'CommandStarted' | 'CommandFinished';

interface Section {
  id: string;
  markers: Record<MarkingPoint, BufferMarker | undefined>;
  decoration: IDecoration | undefined;
  commmandDecorations: IDecoration[];
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

    const oscHandler = (data: string) => {
      const { marker, aid } = parseOsc133(data);

      if (marker === 'A') {
        console.log('PromptStart', aid);
        this.registerMarkingPoint('PromptStart', aid);
      } else if (marker === 'B') {
        this.registerMarkingPoint('PromptEnd', aid);
      } else if (marker === 'C') {
        this.registerMarkingPoint('CommandStarted', aid);
      } else if (marker === 'D') {
        this.registerMarkingPoint('CommandFinished', aid);
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
      (section) => section.markers.CommandStarted !== undefined,
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
            ? this.readFragment(section.markers.PromptEnd, section.markers.CommandStarted)
            : undefined,
          output: section.markers.CommandStarted
            ? this.readFragment(section.markers.CommandStarted, section.markers.CommandFinished)
            : undefined,
        };
      })
      .toArray();

    return sections;
  }

  scrollToSection(sectionId: string) {
    const section = this.sectionsByAid.get(sectionId);
    if (!section || !this.terminal) return;
    this.terminal.scrollToLine(section.markers.PromptStart?.y ?? 0);
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
      CommandStarted: undefined,
      CommandFinished: undefined,
    },
    decoration: undefined,
    commmandDecorations: [],
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

// const markingBorderColor = token('colors.green.surface.border');
const markingBgColor = '#121b17';
const markingRulerColor = '#20573e';

function updateSectionDecorations(term: Terminal, section: Section) {
  updateCommandDecorations(term, section);
  updatePromptDecoration(term, section);
}

function updatePromptDecoration(term: Terminal, section: Section) {
  // Clean up existing decorations
  if (section.decoration) {
    section.decoration.dispose();
    section.decoration = undefined;
  }

  // Check if markers are available
  if (!section.markers.PromptStart) return;

  const activeBuffer = term.buffer.active;

  const y = section.markers.PromptStart.y;
  const marker = term.registerMarker(y - activeBuffer.cursorY - activeBuffer.baseY);
  if (!marker) return;

  const decoration = term.registerDecoration({
    marker,
    layer: 'bottom',
    overviewRulerOptions: {
      color: markingRulerColor,
      position: 'full',
    },
  });
  if (decoration) {
    section.decoration = decoration;
    decoration.onRender((element: HTMLElement) => {
      // TODO: move to recipe ?
      // element.style.borderTop = `2px solid ${markingBorderColor}`;
      // element.style.transform = `translateY(-1px)`;
      element.style.width = 'calc(100% - 5rem)';
      element.dataset.sectionId = section.id;
    });
    decoration.onDispose(() => {
      decoration.marker.dispose();
      const idx = section.commmandDecorations.indexOf(decoration);
      if (idx !== -1) section.commmandDecorations.splice(idx, 1);
    });
  } else {
    marker.dispose();
  }
}

function updateCommandDecorations(term: Terminal, section: Section) {
  // Clean up existing decorations
  for (const decoration of section.commmandDecorations) {
    decoration.dispose();
  }
  section.commmandDecorations = [];

  // Check if markers are available
  if (!section.markers.PromptEnd || !section.markers.CommandStarted) return;

  // Draw new decorations
  const startX = section.markers.PromptEnd.x;
  const startY = section.markers.PromptEnd.y;
  const endY = section.markers.CommandStarted.y - 1;

  // If CommandStarted is at column 0, the prompt content doesn't extend to that line
  if (startY > endY) return;

  const activeBuffer = term.buffer.active;
  for (let y = startY; y <= endY; y++) {
    const x = y === startY ? startX : 0;
    const width = term.cols - x;
    if (width <= 0) continue;

    const marker = term.registerMarker(y - activeBuffer.cursorY - activeBuffer.baseY);
    if (!marker) continue;

    const decoration = term.registerDecoration({
      marker,
      x,
      width,
      layer: 'bottom',
      backgroundColor: markingBgColor,
      overviewRulerOptions: {
        color: markingRulerColor,
        position: 'full',
      },
    });

    if (decoration) {
      section.commmandDecorations.push(decoration);
      decoration.onDispose(() => {
        decoration.marker.dispose();
        const idx = section.commmandDecorations.indexOf(decoration);
        if (idx !== -1) section.commmandDecorations.splice(idx, 1);
      });
    } else {
      marker.dispose();
    }
  }
}
