import { useEffect, useRef, useState } from "react";
import { BAR_DRAW_MS, easeOutBar } from "./bar-motion";
import { drawMode, drawProgress } from "./draw-clock";
import { usePrefersReducedMotion, useVisibleElement } from "./use-ring-motion";

/**
 * Grow Move/Exercise bars from 0 to the imported hourly values.
 * `play` is false while the day is still sliding. `settle` paints the final
 * heights immediately (the day sliding away). Reduced motion snaps to the
 * end and stays there if motion is turned back on. The same signature does not replay.
 */
export function useBarMotion(values: readonly number[], replayKey: string, play: boolean, settle: boolean): { shown: number[]; ref: (el: Element | null) => void } {
  const reduced = usePrefersReducedMotion();
  const { ref, visible } = useVisibleElement();
  const sig = `${replayKey}\0${values.map((v) => (Number.isFinite(v) ? v.toFixed(3) : "0")).join(",")}`;
  const safe = values.map((v) => (Number.isFinite(v) && v > 0 ? v : 0));
  const [shown, setShown] = useState<number[]>(() => (settle || reduced ? safe : safe.map(() => 0)));
  const played = useRef<string | null>(null);

  useEffect(() => {
    const mode = !play && !settle && !reduced
      ? "wait"
      : drawMode({ played: played.current, sig, reduced, visible: play ? visible : false, settle });
    if (mode === "keep") return;
    if (mode === "wait") {
      setShown(safe.map(() => 0));
      return;
    }
    if (mode === "snap" || safe.every((v) => v === 0)) {
      played.current = sig;
      setShown(safe.slice());
      return;
    }
    let raf = 0;
    let origin: number | null = null;
    let finished = false;
    setShown(safe.map(() => 0));
    const tick = (now: number) => {
      if (document.visibilityState === "hidden") {
        origin = null;
        raf = requestAnimationFrame(tick);
        return;
      }
      const step = drawProgress(origin, now, BAR_DRAW_MS);
      origin = step.origin;
      const e = easeOutBar(step.t);
      setShown(safe.map((v) => v * e));
      if (step.t < 1) raf = requestAnimationFrame(tick);
      else {
        finished = true;
        played.current = sig;
      }
    };
    raf = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(raf);
      if (!finished) played.current = null;
    };
    // safe is encoded in sig
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sig, reduced, play, settle, visible]);

  return { shown, ref };
}
