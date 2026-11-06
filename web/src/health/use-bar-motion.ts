import { useEffect, useRef, useState } from "react";
import { BAR_DRAW_MS, barMotion, easeOutBar } from "./bar-motion";
import { usePrefersReducedMotion } from "./use-ring-motion";

/**
 * Grow Move/Exercise bars from 0 to the imported hourly values.
 * `play` is false while the day is still sliding. `settle` paints the final
 * heights immediately (the day sliding away). Reduced motion snaps, including
 * when the preference changes mid-draw. The same signature does not replay.
 */
export function useBarMotion(values: readonly number[], replayKey: string, play: boolean, settle: boolean): number[] {
  const reduced = usePrefersReducedMotion();
  const sig = `${replayKey}\0${values.map((v) => (Number.isFinite(v) ? v.toFixed(3) : "0")).join(",")}`;
  const safe = values.map((v) => (Number.isFinite(v) && v > 0 ? v : 0));
  const [shown, setShown] = useState<number[]>(() => (settle || reduced ? safe : safe.map(() => 0)));
  const played = useRef<string | null>(null);

  useEffect(() => {
    const mode = barMotion(played.current, sig, reduced, play, settle);
    if (mode === "keep") return;
    if (mode === "baseline") {
      setShown(safe.map(() => 0));
      return;
    }
    if (mode === "snap" || safe.every((v) => v === 0)) {
      played.current = sig;
      setShown(safe.slice());
      return;
    }
    played.current = sig;
    let raf = 0;
    let finished = false;
    const t0 = performance.now();
    setShown(safe.map(() => 0));
    const tick = (now: number) => {
      const t = Math.min(1, (now - t0) / BAR_DRAW_MS);
      const e = easeOutBar(t);
      setShown(safe.map((v) => v * e));
      if (t < 1) raf = requestAnimationFrame(tick);
      else finished = true;
    };
    raf = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(raf);
      if (!finished) played.current = null;
    };
    // safe is encoded in sig
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sig, reduced, play, settle]);

  return shown;
}
