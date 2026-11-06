import { useRef } from "react";

/**
 * Grip-only reorder. A press that stays inside 8px is not a drag, so the
 * remove and move buttons beside the grip still receive their clicks.
 */
export function useGripReorder(onReorder: (from: number, to: number) => void) {
  const drag = useRef<{ from: number; id: number; y0: number; moved: boolean; over: number } | null>(null);
  return {
    rowProps: (index: number) => ({ "data-reorder-index": index }),
    gripProps: (index: number) => ({
      "data-grip": "true" as const,
      onPointerDown: (e: React.PointerEvent<HTMLButtonElement>) => {
        if (e.button !== 0) return;
        e.preventDefault();
        e.stopPropagation();
        e.currentTarget.setPointerCapture(e.pointerId);
        drag.current = { from: index, id: e.pointerId, y0: e.clientY, moved: false, over: index };
      },
      onPointerMove: (e: React.PointerEvent<HTMLButtonElement>) => {
        const d = drag.current;
        if (!d || e.pointerId !== d.id) return;
        if (!d.moved && Math.abs(e.clientY - d.y0) < 8) return;
        d.moved = true;
        const hit = document.elementFromPoint(e.clientX, e.clientY)?.closest("[data-reorder-index]");
        if (!hit) return;
        const n = Number(hit.getAttribute("data-reorder-index"));
        if (!Number.isNaN(n)) d.over = n;
      },
      onPointerUp: (e: React.PointerEvent<HTMLButtonElement>) => {
        const d = drag.current;
        if (!d || e.pointerId !== d.id) return;
        drag.current = null;
        e.preventDefault();
        e.stopPropagation();
        if (d.moved && d.over !== d.from) onReorder(d.from, d.over);
      },
      onClick: (e: React.MouseEvent) => {
        e.preventDefault();
        e.stopPropagation();
      },
    }),
  };
}
