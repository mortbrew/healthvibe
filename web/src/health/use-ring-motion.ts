import { useEffect, useRef, useState } from "react";
import { RING_DRAW_MS, easeOutRing, ringMotion } from "./ring-motion";

function usePrefersReducedMotion(): boolean {
  const [reduced, setReduced] = useState(() =>
    typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches,
  );
  useEffect(() => {
    const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
    const on = () => setReduced(mq.matches);
    on();
    mq.addEventListener("change", on);
    return () => mq.removeEventListener("change", on);
  }, []);
  return reduced;
}

function finite(n: number): number {
  return Number.isFinite(n) && n > 0 ? n : 0;
}

/**
 * Drive all rings in one set from the same clock. Replays when `replayKey`
 * or the target fractions change (a new day, or values arriving). Identical
 * rerenders keep the finished pose. Reduced motion shows the end state
 * immediately, including if the preference flips while the draw is running.
 */
export function useRingMotion(targets: readonly number[], replayKey = ""): number[] {
  const reduced = usePrefersReducedMotion();
  const goal = targets.map(finite);
  const sig = `${replayKey}\0${goal.map((n) => n.toFixed(4)).join(",")}`;
  const [shown, setShown] = useState<number[]>(() =>
    typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches
      ? goal
      : goal.map(() => 0),
  );
  const played = useRef<string | null>(null);

  useEffect(() => {
    const mode = ringMotion(played.current, sig, reduced);
    if (mode === "keep") return;
    if (mode === "snap" || goal.every((n) => n === 0)) {
      played.current = sig;
      setShown(goal);
      return;
    }
    played.current = sig;
    let raf = 0;
    let finished = false;
    const t0 = performance.now();
    setShown(goal.map(() => 0));
    const tick = (now: number) => {
      const t = Math.min(1, (now - t0) / RING_DRAW_MS);
      const e = easeOutRing(t);
      setShown(goal.map((v) => v * e));
      if (t < 1) raf = requestAnimationFrame(tick);
      else finished = true;
    };
    raf = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(raf);
      if (!finished) played.current = null;
    };
    // goal is encoded in sig
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sig, reduced]);

  return shown;
}
