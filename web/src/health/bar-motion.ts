/** Move and Exercise bars grow from the baseline. Shorter than the ring draw. */
export const BAR_DRAW_MS = 650;

/** Same cubic ease-out as the rings: fast start, smooth settle, no overshoot. */
export function easeOutBar(t: number): number {
  const x = Math.min(1, Math.max(0, t));
  return 1 - (1 - x) ** 3;
}

export type BarMotion = "baseline" | "snap" | "play" | "keep";

/**
 * `baseline` until the day slide has settled (bars stay at 0).
 * `snap` for reduced motion, or a frozen outgoing day.
 * `play` the first time this date's imported heights are shown.
 * `keep` when that signature already finished, so a rerender does not replay.
 */
export function barMotion(played: string | null, sig: string, reduced: boolean, play: boolean, settle: boolean): BarMotion {
  if (settle || reduced) return "snap";
  if (!play) return "baseline";
  if (played === sig) return "keep";
  return "play";
}

/** Bar heights at ease `t` (0–1). Snap and keep return the imported values. */
export function barHeights(values: readonly number[], t: number, mode: BarMotion): number[] {
  if (mode === "baseline") return values.map(() => 0);
  if (mode === "snap" || mode === "keep") return values.slice();
  const e = easeOutBar(t);
  return values.map((v) => v * e);
}
