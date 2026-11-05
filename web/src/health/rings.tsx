// Fitness-style activity rings (original drawing): thick round caps, dim track, arrows at 12 o'clock.
// A goal is null when the imported data has no ActivitySummary goal for that day:
// the ring then shows only its track (no percentage).
export type RingVals = { move: [number, number | null]; exercise: [number, number | null]; stand: [number, number | null] };

function One({ r, w, frac, color, arrow, dim }: { r: number; w: number; frac: number; color: string; arrow?: number; dim?: boolean }) {
  const c = 2 * Math.PI * r;
  const f = Math.max(0, frac);
  return (
    <g>
      <circle r={r} fill="none" stroke={color} strokeOpacity={dim ? 0.12 : 0.22} strokeWidth={w} />
      {f > 0 && (
        <circle r={r} fill="none" stroke={color} strokeWidth={w} strokeLinecap="round"
          strokeDasharray={`${c * Math.min(f, 1)} ${c}`} transform="rotate(-90)" />
      )}
      {f > 1 && (
        <circle r={r} fill="none" stroke={color} strokeWidth={w} strokeLinecap="round"
          strokeDasharray={`${c * Math.min(f - 1, 0.999)} ${c}`} transform="rotate(-90)" style={{ filter: "drop-shadow(0 0 2px rgba(0,0,0,.6)) brightness(1.15)" }} />
      )}
      {arrow !== undefined && w >= 10 && (
        <g transform={`translate(${w * 0.05},${-r})`} stroke="#000" strokeWidth={w * 0.13} fill="none" strokeLinecap="round" strokeLinejoin="round">
          {arrow === 0 && <path d={`M${-w * 0.22} 0h${w * 0.44}m${-w * 0.18} ${-w * 0.18}l${w * 0.18} ${w * 0.18}l${-w * 0.18} ${w * 0.18}`} />}
          {arrow === 1 && <path d={`M${-w * 0.28} ${-w * 0.18}l${w * 0.18} ${w * 0.18}l${-w * 0.18} ${w * 0.18}M${w * 0.02} ${-w * 0.18}l${w * 0.18} ${w * 0.18}l${-w * 0.18} ${w * 0.18}`} />}
          {arrow === 2 && <path d={`M0 ${w * 0.24}v${-w * 0.46}m${-w * 0.18} ${w * 0.18}l${w * 0.18} ${-w * 0.18}l${w * 0.18} ${w * 0.18}`} />}
        </g>
      )}
    </g>
  );
}

export function FitRings({ v, size = 140, dim, label }: { v: RingVals | null; size?: number; dim?: boolean; label?: string }) {
  const w = size * 0.115, g = size * 0.012;
  const r1 = size / 2 - w / 2, r2 = r1 - w - g, r3 = r2 - w - g;
  const f = (p?: [number, number | null]) => (p && p[1] != null && p[1] > 0 ? p[0] / p[1] : 0);
  return (
    <svg width={size} height={size} viewBox={`${-size / 2} ${-size / 2} ${size} ${size}`} role="img" aria-label={label ?? "Activity rings"} className="fitrings">
      <One r={r1} w={w} frac={f(v?.move)} color="var(--move)" arrow={0} dim={dim} />
      <One r={r2} w={w} frac={f(v?.exercise)} color="var(--exercise)" arrow={1} dim={dim} />
      <One r={r3} w={w} frac={f(v?.stand)} color="var(--stand)" arrow={2} dim={dim} />
    </svg>
  );
}

/** "/goal" suffix for a value, or nothing when the day has no goal. */
export const ofGoal = (g: number | null) => (g == null ? "" : `/${Math.round(g).toLocaleString()}`);

export const ringsOf = (d: { move_kcal: number; move_goal: number | null; exercise_min: number; exercise_goal: number | null; stand_hours: number; stand_goal: number | null }): RingVals => ({
  move: [d.move_kcal, d.move_goal],
  exercise: [d.exercise_min, d.exercise_goal],
  stand: [d.stand_hours, d.stand_goal],
});

