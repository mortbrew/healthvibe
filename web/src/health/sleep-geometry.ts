/** Drawn width of one hypnogram stage. A short stage stays a thin capsule, not a circle. */
export function stageBarWidth(spanPx: number, awake: boolean): number {
  const min = awake ? 3.5 : 5;
  return Math.max(spanPx, min);
}

/** Stadium end cap. A thin bar stays rounded without forcing a square. */
export function capsuleRadius(width: number, height: number): number {
  return Math.min(Math.max(width, 0), Math.max(height, 0)) / 2;
}

/** Week-column segment height. Keep a visible sliver and a small gap between stages. */
export function weekSegmentHeight(spanPx: number): number {
  return Math.max(3, spanPx - 1.6);
}
