// Small dependency-free SVG charts that read colors from CSS variables, so
// they follow the active palette without re-rendering.

import { DayValue } from "../api";

/** One ring. With no goal (none in the imported data) only the track is drawn. */
export function Ring(props: { value: number; goal: number | null; color: string; r: number; width: number }) {
  const c = 2 * Math.PI * props.r;
  const frac = props.goal != null && props.goal > 0 ? props.value / props.goal : 0;
  const shown = Math.min(frac, 1);
  return (
    <g>
      <circle r={props.r} fill="none" stroke={props.color} strokeOpacity={0.2} strokeWidth={props.width} />
      {shown > 0 && (
        <circle
          r={props.r}
          fill="none"
          stroke={props.color}
          strokeWidth={props.width}
          strokeLinecap="round"
          strokeDasharray={`${c * shown} ${c}`}
          transform="rotate(-90)"
        />
      )}
      {frac > 1 && (
        // Overlap marker for goals exceeded, like Apple's second lap.
        <circle
          r={props.r}
          fill="none"
          stroke={props.color}
          strokeWidth={props.width}
          strokeLinecap="round"
          strokeDasharray={`${c * Math.min(frac - 1, 1)} ${c}`}
          transform="rotate(-90)"
          style={{ filter: "brightness(1.25)" }}
        />
      )}
    </g>
  );
}

export function Rings(props: { move: [number, number | null]; exercise: [number, number | null]; stand: [number, number | null]; size?: number }) {
  const size = props.size ?? 180;
  const w = size * 0.11;
  const gap = w * 1.12;
  const r0 = size / 2 - w / 2 - 2;
  return (
    <svg width={size} height={size} viewBox={`${-size / 2} ${-size / 2} ${size} ${size}`} role="img" aria-label="Activity rings">
      <Ring value={props.move[0]} goal={props.move[1]} color="var(--move)" r={r0} width={w} />
      <Ring value={props.exercise[0]} goal={props.exercise[1]} color="var(--exercise)" r={r0 - gap} width={w} />
      <Ring value={props.stand[0]} goal={props.stand[1]} color="var(--stand)" r={r0 - 2 * gap} width={w} />
    </svg>
  );
}

const W = 640;
const fmtDay = (d: string) => {
  const [, m, day] = d.split("-");
  return `${Number(m)}/${Number(day)}`;
};

export function Bars(props: { data: DayValue[]; color: string; goal?: number; height?: number; unit?: string }) {
  const H = props.height ?? 160;
  const pad = { l: 8, r: 8, t: 10, b: 22 };
  const max = Math.max(1, props.goal ?? 0, ...props.data.map((d) => d.value)) * 1.08;
  const n = Math.max(props.data.length, 1);
  const bw = (W - pad.l - pad.r) / n;
  const y = (v: number) => pad.t + (H - pad.t - pad.b) * (1 - v / max);
  return (
    <svg viewBox={`0 0 ${W} ${H}`} className="chart" role="img">
      {props.goal !== undefined && (
        <line x1={pad.l} x2={W - pad.r} y1={y(props.goal)} y2={y(props.goal)} stroke="var(--muted)" strokeDasharray="4 4" strokeOpacity={0.6} />
      )}
      {props.data.map((d, i) => (
        <g key={d.date}>
          <rect
            x={pad.l + i * bw + bw * 0.15}
            y={y(d.value)}
            width={bw * 0.7}
            height={Math.max(0, H - pad.b - y(d.value))}
            rx={Math.min(4, bw * 0.3)}
            fill={props.color}
            opacity={i === props.data.length - 1 ? 1 : 0.75}
          >
            <title>{`${d.date}: ${Math.round(d.value).toLocaleString()}${props.unit ? " " + props.unit : ""}`}</title>
          </rect>
          {(i % Math.ceil(n / 6) === 0 || i === n - 1) && (
            <text x={pad.l + i * bw + bw / 2} y={H - 6} textAnchor="middle" className="axis">{fmtDay(d.date)}</text>
          )}
        </g>
      ))}
    </svg>
  );
}

export function Lines(props: {
  series: { data: DayValue[]; color: string; label: string }[];
  band?: { date: string; lo: number; hi: number }[];
  bandColor?: string;
  height?: number;
  decimals?: number;
}) {
  const H = props.height ?? 160;
  const pad = { l: 34, r: 10, t: 10, b: 22 };
  const all = [...props.series.flatMap((s) => s.data.map((d) => d.value)), ...(props.band ?? []).flatMap((b) => [b.lo, b.hi])];
  if (all.length === 0) return null;
  let lo = Math.min(...all);
  let hi = Math.max(...all);
  const span = hi - lo || 1;
  lo -= span * 0.1;
  hi += span * 0.1;
  const dates = Array.from(new Set([...props.series.flatMap((s) => s.data.map((d) => d.date)), ...(props.band ?? []).map((b) => b.date)])).sort();
  const x = (d: string) => pad.l + ((W - pad.l - pad.r) * dates.indexOf(d)) / Math.max(dates.length - 1, 1);
  const y = (v: number) => pad.t + (H - pad.t - pad.b) * (1 - (v - lo) / (hi - lo));
  const fmt = (v: number) => v.toFixed(props.decimals ?? 0);
  const ticks = [lo + (hi - lo) * 0.15, (lo + hi) / 2, hi - (hi - lo) * 0.15];
  return (
    <svg viewBox={`0 0 ${W} ${H}`} className="chart" role="img">
      {ticks.map((t) => (
        <g key={t}>
          <line x1={pad.l} x2={W - pad.r} y1={y(t)} y2={y(t)} stroke="var(--border)" />
          <text x={pad.l - 6} y={y(t) + 4} textAnchor="end" className="axis">{fmt(t)}</text>
        </g>
      ))}
      {props.band && props.band.length > 0 && (
        <path
          d={
            props.band.map((b, i) => `${i ? "L" : "M"}${x(b.date)},${y(b.hi)}`).join("") +
            [...props.band].reverse().map((b) => `L${x(b.date)},${y(b.lo)}`).join("") +
            "Z"
          }
          fill={props.bandColor ?? "var(--accent)"}
          opacity={0.16}
        />
      )}
      {props.series.map((s) => (
        <g key={s.label}>
          <path
            d={s.data.map((d, i) => `${i ? "L" : "M"}${x(d.date)},${y(d.value)}`).join("")}
            fill="none"
            stroke={s.color}
            strokeWidth={2.5}
            strokeLinejoin="round"
            strokeLinecap="round"
          />
          {s.data.length > 0 && (
            <circle cx={x(s.data[s.data.length - 1].date)} cy={y(s.data[s.data.length - 1].value)} r={4} fill={s.color} />
          )}
        </g>
      ))}
      {dates.map((d, i) =>
        i % Math.ceil(dates.length / 6) === 0 || i === dates.length - 1 ? (
          <text key={d} x={x(d)} y={H - 6} textAnchor="middle" className="axis">{fmtDay(d)}</text>
        ) : null,
      )}
    </svg>
  );
}

export const STAGES = [
  { key: "Awake", label: "Awake", color: "var(--awake)" },
  { key: "AsleepREM", label: "REM", color: "var(--rem)" },
  { key: "AsleepCore", label: "Core", color: "var(--core)" },
  { key: "AsleepDeep", label: "Deep", color: "var(--deep)" },
];

/** Legend rows for a night: always Awake/REM/Core/Deep; Unspecified and In Bed when present.
 *  Asleep total = rem + core + deep + unspecified (Awake / In Bed excluded). */
export type NightMins = {
  awake_min: number;
  rem_min: number;
  core_min: number;
  deep_min: number;
  unspecified_min: number;
  in_bed_min: number;
  asleep_min: number;
};

export function sleepLegendRows(n: NightMins): { key: string; label: string; color: string; minutes: number; asleep: boolean }[] {
  const rows = [
    { key: "Awake", label: "Awake", color: "var(--awake)", minutes: n.awake_min, asleep: false },
    { key: "AsleepREM", label: "REM", color: "var(--rem)", minutes: n.rem_min, asleep: true },
    { key: "AsleepCore", label: "Core", color: "var(--core)", minutes: n.core_min, asleep: true },
    { key: "AsleepDeep", label: "Deep", color: "var(--deep)", minutes: n.deep_min, asleep: true },
  ];
  if (n.unspecified_min > 0) {
    rows.push({
      key: "AsleepUnspecified",
      label: "Asleep Unspecified",
      color: "var(--unspecified)",
      minutes: n.unspecified_min,
      asleep: true,
    });
  }
  if (n.in_bed_min > 0) {
    rows.push({
      key: "InBed",
      label: "In Bed",
      color: "var(--inbed)",
      minutes: n.in_bed_min,
      asleep: false,
    });
  }
  return rows;
}

export function asleepFromLegend(n: NightMins): number {
  return n.rem_min + n.core_min + n.deep_min + n.unspecified_min;
}

/** Hypnogram: stages as rows over the night, Sleep Stats style. */
export function Hypnogram(props: { segments: { stage: string; start: string; end: string }[] }) {
  const H = 150;
  const pad = { l: 56, r: 10, t: 8, b: 22 };
  const t = (s: string) => new Date(s.replace(" ", "T")).getTime();
  const segs = props.segments.filter((s) => STAGES.some((st) => st.key === s.stage) || s.stage === "AsleepUnspecified");
  if (segs.length === 0) return null;
  const t0 = Math.min(...segs.map((s) => t(s.start)));
  const t1 = Math.max(...segs.map((s) => t(s.end)));
  const x = (v: number) => pad.l + ((W - pad.l - pad.r) * (v - t0)) / Math.max(t1 - t0, 1);
  const rowH = (H - pad.t - pad.b) / STAGES.length;
  // Unstaged "asleep" (older watches) is drawn on the Core row.
  const row = (stage: string) => {
    const i = STAGES.findIndex((s) => s.key === stage);
    return i < 0 ? 2 : i;
  };
  const hours: number[] = [];
  for (let h = Math.ceil(t0 / 3_600_000) * 3_600_000; h <= t1; h += 3_600_000) hours.push(h);
  const step = Math.ceil(hours.length / 7);
  return (
    <svg viewBox={`0 0 ${W} ${H}`} className="chart" role="img" aria-label="Sleep stages last night">
      {STAGES.map((s, i) => (
        <text key={s.key} x={0} y={pad.t + rowH * i + rowH / 2 + 4} className="axis">{s.label}</text>
      ))}
      {segs.map((s, i) => {
        const r = row(s.stage);
        const color =
          s.stage === "AsleepUnspecified"
            ? "var(--unspecified)"
            : (STAGES[r]?.color ?? "var(--core)");
        return (
          <rect key={i} x={x(t(s.start))} y={pad.t + rowH * r + 3} width={Math.max(1.5, x(t(s.end)) - x(t(s.start)))} height={rowH - 6} rx={3} fill={color} />
        );
      })}
      {hours.map((h, i) =>
        i % step === 0 ? (
          <text key={h} x={x(h)} y={H - 6} textAnchor="middle" className="axis">
            {new Date(h).toLocaleTimeString([], { hour: "numeric" })}
          </text>
        ) : null,
      )}
    </svg>
  );
}
