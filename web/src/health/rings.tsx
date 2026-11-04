// Fitness-style activity rings (original drawing): thick round caps, dim track, arrows at 12 o'clock.
export type RingVals = { move: [number, number]; exercise: [number, number]; stand: [number, number] };

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
  const f = (p?: [number, number]) => (p && p[1] > 0 ? p[0] / p[1] : 0);
  return (
    <svg width={size} height={size} viewBox={`${-size / 2} ${-size / 2} ${size} ${size}`} role="img" aria-label={label ?? "Activity rings"} className="fitrings">
      <One r={r1} w={w} frac={f(v?.move)} color="var(--move)" arrow={0} dim={dim} />
      <One r={r2} w={w} frac={f(v?.exercise)} color="var(--exercise)" arrow={1} dim={dim} />
      <One r={r3} w={w} frac={f(v?.stand)} color="var(--stand)" arrow={2} dim={dim} />
    </svg>
  );
}

export const ringsOf = (d: { move_kcal: number; move_goal: number; exercise_min: number; exercise_goal: number; stand_hours: number; stand_goal: number }): RingVals => ({
  move: [d.move_kcal, d.move_goal],
  exercise: [d.exercise_min, d.exercise_goal],
  stand: [d.stand_hours, d.stand_goal],
});

const PAL: Record<string, [string, string]> = {
  move: ["#ff2d55", "#ff8a3d"], exercise: ["#9cf000", "#2bd96b"], stand: ["#33e8ff", "#2d8cff"],
  all: ["#ff2d55", "#33e8ff"], steps: ["#b18cff", "#6e4bff"], workout: ["#ffd23d", "#ff8a1f"], record: ["#ffe27a", "#d9a21b"],
};

/** Original award badges: ring medallion, hexagon, or round medal, colored by category. */
export function Badge({ color, shape, mark, size = 96, dim }: { color: string; shape: string; mark?: string; size?: number; dim?: boolean }) {
  const [a, b] = PAL[color] ?? PAL.move;
  const id = `bd-${color}-${shape}`;
  const text = mark ? <text x="50" y="58" textAnchor="middle" className="badge-mark" fontSize={mark.length > 3 ? 15 : mark.length > 2 ? 19 : 24}>{mark}</text> : null;
  return (
    <svg width={size} height={size} viewBox="0 0 100 100" aria-hidden className={dim ? "badge dim" : "badge"}>
      <defs>
        <linearGradient id={id} x1="0" y1="0" x2="1" y2="1"><stop offset="0" stopColor={a} /><stop offset="1" stopColor={b} /></linearGradient>
        <linearGradient id={id + "m"} x1="0" y1="0" x2="0" y2="1"><stop offset="0" stopColor="#fafafa" /><stop offset=".5" stopColor="#a9abb3" /><stop offset="1" stopColor="#e9eaee" /></linearGradient>
      </defs>
      {shape === "hex" && (
        <>
          <path d="M50 3 91 26.5v47L50 97 9 73.5v-47Z" fill={`url(#${id}m)`} />
          <path d="M50 10 85 30v40L50 90 15 70V30Z" fill={`url(#${id})`} />
          <path d="M50 18 78 34v32L50 82 22 66V34Z" fill="#000" opacity=".22" />
          {text}
        </>
      )}
      {shape === "medal" && (
        <>
          <circle cx="50" cy="50" r="46" fill={`url(#${id}m)`} />
          <circle cx="50" cy="50" r="40" fill={`url(#${id})`} />
          {Array.from({ length: 12 }, (_, i) => <path key={i} d="M50 13v8" stroke="#fff" strokeOpacity=".45" strokeWidth="3" strokeLinecap="round" transform={`rotate(${i * 30} 50 50)`} />)}
          <circle cx="50" cy="50" r="27" fill="#000" opacity=".25" />
          {text}
        </>
      )}
      {shape !== "hex" && shape !== "medal" && (
        <>
          <circle cx="50" cy="50" r="46" fill="#1c1c1e" />
          <circle cx="50" cy="50" r="34" fill="none" stroke={`url(#${id})`} strokeWidth="14" />
          <circle cx="50" cy="50" r="46" fill="none" stroke={`url(#${id}m)`} strokeWidth="3" />
          {mark ? text : <path d="M41 50l7 7 13-14" fill="none" stroke="#fff" strokeWidth="5" strokeLinecap="round" strokeLinejoin="round" />}
        </>
      )}
    </svg>
  );
}
