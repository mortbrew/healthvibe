// Activity-ring motion helpers. Progress is value/goal from imported or manual
// goals only. A missing or non-positive goal is an empty track, not a guessed
// percentage. The ease is a cubic ease-out (fast start, smooth settle, no
// overshoot) in the spirit of the Apple Fitness recording, not a traced curve.

export const RING_DRAW_MS = 1000;

/** Portion of the goal to draw. Zero when there is nothing honest to draw. */
export function ringFraction(value: number, goal: number | null | undefined): number {
  if (goal == null || !(goal > 0) || !Number.isFinite(value) || value <= 0) return 0;
  return value / goal;
}

/**
 * How to stroke `progress`, which may be greater than one lap.
 * `base` is the first circle (0–1). `extra` is the overlap lap past a full
 * ring (0–1). An exact extra lap (2×, 3×, …) keeps a full overlap so the
 * ring does not snap back to a single uncapped stroke.
 */
export function ringLaps(progress: number): { base: number; extra: number } {
  const p = Number.isFinite(progress) && progress > 0 ? progress : 0;
  if (p <= 1) return { base: p, extra: 0 };
  const frac = p - Math.floor(p);
  return { base: 1, extra: frac === 0 ? 1 : frac };
}

/** Turns clockwise from 12 o'clock (0–1) where the leading tip sits. */
export function tipTurns(progress: number): number {
  const { base, extra } = ringLaps(progress);
  if (extra > 0) return extra >= 1 ? 0 : extra;
  return base >= 1 ? 0 : base;
}

/** Cubic ease-out. f(0)=0, f(1)=1, no overshoot, steeper at the start. */
export function easeOutRing(t: number): number {
  const x = Math.min(1, Math.max(0, t));
  return 1 - (1 - x) ** 3;
}

export type RingMotion = "snap" | "play" | "keep";

/**
 * `snap` when reduced motion is on (show the end state).
 * `play` when this value/day signature has not finished.
 * `keep` when the same signature already finished, so a rerender does not replay.
 */
export function ringMotion(playedSig: string | null, sig: string, reduced: boolean): RingMotion {
  if (reduced) return "snap";
  if (playedSig === sig) return "keep";
  return "play";
}
