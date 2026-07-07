import { useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react';
import { css } from 'styled-system/css';
import { sectionConnector } from 'styled-system/recipes';
import { token } from 'styled-system/tokens';
import { useDebouncedCallback } from '@/app/shared/use-debounced-callback';
import { UpdateMatchingContext } from './context';

export interface SectionMatchingProps {}

interface Connection {
  id: string;
  d: string;
}

const STROKE = token('colors.sectionConnector');

const connectorStyle = sectionConnector();

function buildPath(startX: number, endX: number, y1: number, y2: number): string {
  const midX = (startX + endX) / 2;
  return `M ${startX} ${y1} C ${midX} ${y1}, ${midX} ${y2}, ${endX} ${y2}`;
}

export function SectionMatching() {
  const svgRef = useRef<SVGSVGElement>(null);
  const [connections, setConnections] = useState<Connection[]>([]);

  const refresh = useCallback(() => {
    const svg = svgRef.current;
    if (!svg) return;
    const container = svg.parentElement;
    if (!container) return;

    const svgRect = svg.getBoundingClientRect();

    const termElements = container.querySelectorAll<HTMLElement>('[data-sect-id]');
    const termBySection = new Map<string, HTMLElement>();
    for (const termEl of termElements) {
      const sectId = termEl.dataset.sectId;
      if (sectId) termBySection.set(sectId, termEl);
      termEl.classList.remove(connectorStyle);
    }

    const chatElements = container.querySelectorAll<HTMLElement>('[data-term-sect-id]');

    const next: Connection[] = [];
    for (const chatEl of chatElements) {
      const sectId = chatEl.dataset.termSectId;
      if (!sectId) continue;

      const termEl = termBySection.get(sectId);
      if (!termEl) continue;

      const termRect = termEl.getBoundingClientRect();
      const startX = termRect.right - svgRect.left + 1;
      const termY = termRect.top - svgRect.top + 1;

      if (termY < 0 || termY > svgRect.height) continue;

      const chatRect = chatEl.getBoundingClientRect();
      const endX = chatRect.left - svgRect.left - 1;
      const chatY = chatRect.top - svgRect.top + 1;
      next.push({
        id: `cn:${sectId}`,
        d: buildPath(startX, endX, termY, chatY),
      });

      termEl.classList.add(connectorStyle);
      console.log('add', next.at(-1));
    }

    setConnections(next);
  }, []);

  const debouncedRefresh = useDebouncedCallback(refresh, 0);
  useUpdateMatchingListener(debouncedRefresh);

  const paths = useMemo(
    () =>
      connections.map((conn) => (
        <path key={conn.id} d={conn.d} stroke={STROKE} strokeWidth={2} strokeDasharray="2 2" />
      )),
    [connections],
  );

  return (
    <svg
      ref={svgRef}
      aria-hidden="true"
      fill="none"
      className={css({
        pointerEvents: 'none',
        position: 'absolute',
        width: 'full',
        height: 'full',
      })}
    >
      {paths}
    </svg>
  );
}

export function useUpdateMatchingListener(callback: () => void): void {
  const ctx = useContext(UpdateMatchingContext);
  useEffect(() => {
    if (!ctx) return;
    return ctx.subscribe(callback);
  }, [ctx, callback]);
}
