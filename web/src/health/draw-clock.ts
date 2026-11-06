export type DrawMode = "snap" | "wait" | "play" | "keep";

/**
 * When a draw is allowed to run.
 * Reduced motion and an outgoing slide snap to the end and count as finished,
 * so leaving reduced motion does not restart from zero.
 * An off-screen chart waits at the baseline until a real slice of it is in
 * view. A chart that already finished stays put if it scrolls away.
 */
export function drawMode(opts: {
  played: string | null;
  sig: string;
  reduced: boolean;
  visible: boolean;
  settle: boolean;
}): DrawMode {
  if (opts.settle || opts.reduced) return "snap";
  if (opts.played === opts.sig) return "keep";
  if (!opts.visible) return "wait";
  return "play";
}

/**
 * Progress from the first painted frame. `origin` is null until that frame,
 * so a callback WebKit deferred for a long time still starts at 0 instead of
 * skipping to the end.
 */
export function drawProgress(origin: number | null, now: number, duration: number): { origin: number; t: number } {
  const start = origin == null ? now : origin;
  const span = duration > 0 ? duration : 1;
  return { origin: start, t: Math.min(1, Math.max(0, (now - start) / span)) };
}

export type DrawClock = { shown: number; played: string | null; origin: number | null };

/**
 * One step of the ring/bar clock the hooks use.
 * `now` is a frame timestamp, or null when no frame should advance the draw.
 */
export function advanceDraw(clock: DrawClock, input: {
  sig: string;
  reduced: boolean;
  visible: boolean;
  settle: boolean;
  now: number | null;
  duration: number;
}): DrawClock {
  const mode = drawMode({
    played: clock.played,
    sig: input.sig,
    reduced: input.reduced,
    visible: input.visible,
    settle: input.settle,
  });
  if (mode === "keep") return clock;
  if (mode === "wait") return { shown: 0, played: null, origin: null };
  if (mode === "snap") return { shown: 1, played: input.sig, origin: null };
  if (input.now == null) return { shown: 0, played: null, origin: null };
  const step = drawProgress(clock.origin, input.now, input.duration);
  const done = step.t >= 1;
  return { shown: step.t, played: done ? input.sig : null, origin: done ? null : step.origin };
}

/** A chart counts as on screen only when a real slice of its box is inside the viewport. */
export function boxInView(box: { top: number; bottom: number; left: number; right: number; width: number; height: number }, viewport: { width: number; height: number }): boolean {
  if (box.width < 8 || box.height < 8 || viewport.width < 8 || viewport.height < 8) return false;
  const visH = Math.min(box.bottom, viewport.height) - Math.max(box.top, 0);
  const visW = Math.min(box.right, viewport.width) - Math.max(box.left, 0);
  return visH >= Math.min(40, box.height * 0.45) && visW >= 8;
}
