import { useCallback, useEffect, useRef, useState } from "react";
import { drawMode, drawProgress } from "./draw-clock";
import { RING_DRAW_MS, easeOutRing } from "./ring-motion";

export function usePrefersReducedMotion(): boolean {
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

/** True once `el` is on screen and the document is visible. Hidden time does not count. */
export function useVisibleElement() {
  const [node, setNode] = useState<Element | null>(null);
  const [visible, setVisible] = useState(false);
  const hit = useRef(false);
  const ref = useCallback((el: Element | null) => setNode(el), []);
  useEffect(() => {
    if (!node) {
      hit.current = false;
      setVisible(false);
      return;
    }
    const publish = () => setVisible(hit.current && document.visibilityState === "visible");
    let io: IntersectionObserver | undefined;
    if (typeof IntersectionObserver === "undefined") hit.current = true;
    else {
      io = new IntersectionObserver((entries) => {
        hit.current = entries.some((e) => e.isIntersecting);
        publish();
      }, { threshold: [0, 0.05] });
      io.observe(node);
    }
    publish();
    document.addEventListener("visibilitychange", publish);
    // Some WebKit builds never deliver IntersectionObserver for an SVG.
    // If the document is showing, start the draw anyway so it cannot stay blank.
    const fallback = window.setTimeout(() => {
      if (document.visibilityState === "visible") {
        hit.current = true;
        publish();
      }
    }, 350);
    return () => {
      window.clearTimeout(fallback);
      io?.disconnect();
      document.removeEventListener("visibilitychange", publish);
    };
  }, [node]);
  return { ref, visible };
}

/**
 * Drive all rings in one set from the same clock. The draw starts on the first
 * frame the SVG is actually on screen, so a deferred mobile callback cannot
 * finish the ring before it is visible. Replays when `replayKey` or the
 * fractions change. The same signature does not replay. Reduced motion snaps,
 * including a live change, and does not count as a finished play. `settle`
 * snaps the day that is sliding away.
 */
export function useRingMotion(targets: readonly number[], replayKey = "", settle = false): { shown: number[]; ref: (el: Element | null) => void } {
  const reduced = usePrefersReducedMotion();
  const { ref, visible } = useVisibleElement();
  const goal = targets.map(finite);
  const sig = `${replayKey}\0${goal.map((n) => n.toFixed(4)).join(",")}`;
  const snap0 = settle || (typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches);
  const [shown, setShown] = useState<number[]>(() => (snap0 ? goal : goal.map(() => 0)));
  const played = useRef<string | null>(null);

  useEffect(() => {
    const mode = drawMode({ played: played.current, sig, reduced, visible, settle });
    if (mode === "keep") return;
    if (mode === "wait") {
      setShown(goal.map(() => 0));
      return;
    }
    if (mode === "snap" || goal.every((n) => n === 0)) {
      // A snap is not a finished draw. Turning motion back on must still play.
      played.current = goal.every((n) => n === 0) && !reduced ? sig : null;
      setShown(goal);
      return;
    }
    let raf = 0;
    let origin: number | null = null;
    let finished = false;
    setShown(goal.map(() => 0));
    const tick = (now: number) => {
      if (document.visibilityState === "hidden") {
        origin = null;
        raf = requestAnimationFrame(tick);
        return;
      }
      const step = drawProgress(origin, now, RING_DRAW_MS);
      origin = step.origin;
      const e = easeOutRing(step.t);
      setShown(goal.map((v) => v * e));
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
    // goal is encoded in sig
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sig, reduced, settle, visible]);

  return { shown, ref };
}
