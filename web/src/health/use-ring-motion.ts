import { useCallback, useEffect, useRef, useState } from "react";
import { boxInView, drawMode, drawProgress } from "./draw-clock";
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

function elementInView(el: Element): boolean {
  if (document.visibilityState === "hidden") return false;
  const r = el.getBoundingClientRect();
  return boxInView(r, {
    width: window.innerWidth || document.documentElement.clientWidth,
    height: window.innerHeight || document.documentElement.clientHeight,
  });
}

/** True only while a real slice of `el` is inside the viewport. */
export function useVisibleElement() {
  const [node, setNode] = useState<Element | null>(null);
  const [visible, setVisible] = useState(false);
  const ref = useCallback((el: Element | null) => setNode(el), []);
  useEffect(() => {
    if (!node) {
      setVisible(false);
      return;
    }
    const publish = () => setVisible(elementInView(node));
    let io: IntersectionObserver | undefined;
    if (typeof IntersectionObserver !== "undefined") {
      io = new IntersectionObserver(() => publish(), { threshold: [0, 0.25, 0.5, 1] });
      io.observe(node);
    }
    publish();
    document.addEventListener("scroll", publish, true);
    window.addEventListener("resize", publish);
    document.addEventListener("visibilitychange", publish);
    return () => {
      io?.disconnect();
      document.removeEventListener("scroll", publish, true);
      window.removeEventListener("resize", publish);
      document.removeEventListener("visibilitychange", publish);
    };
  }, [node]);
  return { ref, visible };
}

/**
 * Drive all rings in one set from the same clock. The draw starts on the first
 * frame the SVG is actually on screen, so a deferred mobile callback cannot
 * finish the ring before it is visible. Replays when `replayKey` or the
 * fractions change. The same signature does not replay. Reduced motion snaps
 * to the end and stays there if motion is turned back on. `settle` snaps the
 * day that is sliding away.
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
      played.current = sig;
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
