export type DrawMode = "snap" | "wait" | "play" | "keep";

/**
 * When a draw is allowed to run.
 * Reduced motion and an outgoing slide snap to the end.
 * Off-screen or still-sliding charts wait at the baseline so the draw is not
 * already finished when the phone first paints them.
 * A finished signature does not replay. Reduced motion must not count as finished,
 * so turning motion back on plays the draw.
 */
export function drawMode(opts: {
  played: string | null;
  sig: string;
  reduced: boolean;
  visible: boolean;
  settle: boolean;
}): DrawMode {
  if (opts.settle || opts.reduced) return "snap";
  if (!opts.visible) return "wait";
  if (opts.played === opts.sig) return "keep";
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
