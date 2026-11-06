/** Calendar steps for the activity day strip. ISO dates compare in order. */

export const DAY_SLIDE_MS = 280;
/** A little longer than the slide so the draw starts after the transform settles. */
export const DAY_SETTLE_MS = 340;

export function shiftDate(iso: string, days: number): string {
  const x = new Date(iso + "T12:00:00");
  x.setDate(x.getDate() + days);
  return x.toISOString().slice(0, 10);
}

/** Monday of the week that contains `iso` (the strip is Monday–Sunday). */
export function mondayOf(iso: string): string {
  const dt = new Date(iso + "T12:00:00");
  return shiftDate(iso, -((dt.getDay() + 6) % 7));
}

export function dayDirection(from: string, to: string): -1 | 0 | 1 {
  if (from === to) return 0;
  return to > from ? 1 : -1;
}

/** Same rule as the week strip: a later calendar day draws no progress. */
export function isFutureDay(date: string, today: string): boolean {
  return date > today;
}

/**
 * Several day clicks before the slide finishes: keep the last settled day as
 * the origin and move only to the newest request. Intermediate dates are dropped.
 */
export function rapidSlide(settled: string, requests: readonly string[]): { from: string; to: string; dir: -1 | 0 | 1 } {
  const to = requests.length ? requests[requests.length - 1] : settled;
  return { from: settled, to, dir: dayDirection(settled, to) };
}

/** Draw waits until the slide is idle. Reduced motion snaps and does not slide. */
export function replayGate(phase: "idle" | "run", reduced: boolean): "wait" | "snap" | "play" {
  if (reduced) return "snap";
  if (phase === "run") return "wait";
  return "play";
}
