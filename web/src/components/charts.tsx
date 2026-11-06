// Small dependency-free SVG charts that read colors from CSS variables, so
// they follow the active palette without re-rendering.

import { useId } from "react";
import { DayValue } from "../api";
import { ringFraction, ringLaps } from "../health/ring-motion";
import { capsuleRadius, stageBarWidth, weekSegmentHeight } from "../health/sleep-geometry";
import { useRingMotion } from "../health/use-ring-motion";

/** One ring stroke. `progress` may exceed 1; the extra lap is the overlap, not a 100% cap. */
export function Ring(props: { progress: number; color: string; r: number; width: number }) {
  const c = 2 * Math.PI * props.r;
  const { base, extra } = ringLaps(props.progress);
  const dash = (portion: number) => `${Math.max(0, portion) * c} ${c}`;
  return (
    <g>
      <circle r={props.r} fill="none" stroke={props.color} strokeOpacity={0.2} strokeWidth={props.width} />
      {base > 0 && (
        <circle
          r={props.r}
          fill="none"
          stroke={props.color}
          strokeWidth={props.width}
          strokeLinecap="round"
          strokeDasharray={dash(base)}
          transform="rotate(-90)"
        />
      )}
      {extra > 0 && (
        <circle
          r={props.r}
          fill="none"
          stroke={props.color}
          strokeWidth={props.width}
          strokeLinecap="round"
          strokeDasharray={dash(extra)}
          transform="rotate(-90)"
          style={{ filter: "drop-shadow(0 0 2px rgba(0,0,0,.45)) brightness(1.12)" }}
        />
      )}
    </g>
  );
}

export function Rings(props: { move: [number, number | null]; exercise: [number, number | null]; stand: [number, number | null]; size?: number; replayKey?: string }) {
  const size = props.size ?? 180;
  const w = size * 0.11;
  const gap = w * 1.12;
  const r0 = size / 2 - w / 2 - 2;
  const shown = useRingMotion([
    ringFraction(props.move[0], props.move[1]),
    ringFraction(props.exercise[0], props.exercise[1]),
    ringFraction(props.stand[0], props.stand[1]),
  ], props.replayKey ?? "");
  return (
    <svg width={size} height={size} viewBox={`${-size / 2} ${-size / 2} ${size} ${size}`} role="img" aria-label="Activity rings" data-ring-progress={shown.map((n) => n.toFixed(3)).join(",")}>
      <Ring progress={shown[0] ?? 0} color="var(--move)" r={r0} width={w} />
      <Ring progress={shown[1] ?? 0} color="var(--exercise)" r={r0 - gap} width={w} />
      <Ring progress={shown[2] ?? 0} color="var(--stand)" r={r0 - 2 * gap} width={w} />
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
  { key: "Awake", label: "Awake", color: "#ff5e4d" },
  { key: "AsleepREM", label: "REM", color: "#64d2ff" },
  { key: "AsleepCore", label: "Core", color: "#3f8ef7" },
  { key: "AsleepDeep", label: "Deep", color: "#6c5ce7" },
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
  const tone = (key: string) => STAGES.find((s) => s.key === key)?.color ?? "var(--unspecified)";
  const rows = [
    { key: "Awake", label: "Awake", color: tone("Awake"), minutes: n.awake_min, asleep: false },
    { key: "AsleepREM", label: "REM", color: tone("AsleepREM"), minutes: n.rem_min, asleep: true },
    { key: "AsleepCore", label: "Core", color: tone("AsleepCore"), minutes: n.core_min, asleep: true },
    { key: "AsleepDeep", label: "Deep", color: tone("AsleepDeep"), minutes: n.deep_min, asleep: true },
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

/**
 * Apple Health day / More Stages chart: rounded stage bars centered on
 * horizontal grid lines, with a thin connector at each stage change.
 * Unstaged "asleep" (older watches) is drawn on the Core row.
 */
export function Hypnogram(props: { segments: { stage: string; start: string; end: string }[] }) {
  const uid = useId().replace(/:/g, "");
  // Phone-like aspect. A 640-wide viewBox made the lanes short and the bars fat.
  const plotW = 360;
  const H = 268;
  const pad = { l: 52, r: 8, t: 12, b: 26 };
  const segs = props.segments
    .filter((s) => STAGES.some((st) => st.key === s.stage) || s.stage === "AsleepUnspecified")
    .slice()
    .sort((a, b) => parseLocal(a.start) - parseLocal(b.start) || parseLocal(a.end) - parseLocal(b.end));
  if (segs.length === 0) return null;
  const t0 = Math.min(...segs.map((s) => parseLocal(s.start)));
  const t1 = Math.max(...segs.map((s) => parseLocal(s.end)));
  const ticks: number[] = [];
  const cursor = new Date(t0);
  cursor.setMinutes(0, 0, 0);
  if (cursor.getHours() % 2 !== 0) cursor.setHours(cursor.getHours() - 1);
  const axisEnd = new Date(t1);
  axisEnd.setMinutes(0, 0, 0);
  if (axisEnd.getTime() < t1) axisEnd.setHours(axisEnd.getHours() + 1);
  if (axisEnd.getHours() % 2 !== 0) axisEnd.setHours(axisEnd.getHours() + 1);
  while (ticks.length < 8 && cursor.getTime() <= axisEnd.getTime()) {
    ticks.push(cursor.getTime());
    cursor.setHours(cursor.getHours() + 2);
  }
  const axis0 = Math.min(t0, ticks[0] ?? t0);
  const axis1 = Math.max(t1, ticks[ticks.length - 1] ?? t1);
  const x = (v: number) => pad.l + ((plotW - pad.l - pad.r) * (v - axis0)) / Math.max(axis1 - axis0, 1);
  const rowH = (H - pad.t - pad.b) / STAGES.length;
  const barH = (stage: string) => (stage === "Awake" ? 14 : 18);
  const row = (stage: string) => {
    const i = STAGES.findIndex((s) => s.key === stage);
    return i < 0 ? 2 : i;
  };
  const colorOf = (stage: string) =>
    stage === "AsleepUnspecified" ? "var(--unspecified)" : (STAGES[row(stage)]?.color ?? "var(--core)");
  const yC = (stage: string) => pad.t + rowH * row(stage) + rowH / 2;
  const fmtTick = (ms: number) => {
    const d = new Date(ms);
    return `${String(d.getHours()).padStart(2, "0")}:00`;
  };
  const bars = segs.map((s) => {
    const x1 = x(parseLocal(s.start));
    const x2 = x(parseLocal(s.end));
    const h = barH(s.stage);
    const w = stageBarWidth(x2 - x1, s.stage === "Awake");
    const mid = (x1 + x2) / 2;
    return { stage: s.stage, left: mid - w / 2, right: mid + w / 2, y: yC(s.stage), h, w };
  });
  const links = bars.slice(1).flatMap((b, i) => {
    const prev = bars[i];
    if (row(prev.stage) === row(b.stage)) return [];
    const xj = (prev.right + b.left) / 2;
    const down = b.y >= prev.y;
    const top = down ? prev.y + prev.h / 2 : b.y + b.h / 2;
    const bot = down ? b.y - b.h / 2 : prev.y - prev.h / 2;
    if (bot - top < 1) return [];
    return [{
      id: `${uid}-c${i}`,
      xj,
      top,
      bot,
      topColor: down ? colorOf(prev.stage) : colorOf(b.stage),
      botColor: down ? colorOf(b.stage) : colorOf(prev.stage),
    }];
  });
  return (
    <svg viewBox={`0 0 ${plotW} ${H}`} className="chart hypnogram" role="img" aria-label="Sleep stages last night">
      <defs>
        {links.map((l) => (
          <linearGradient key={l.id} id={l.id} gradientUnits="userSpaceOnUse" x1={l.xj} y1={l.top} x2={l.xj} y2={l.bot}>
            <stop offset="0%" stopColor={l.topColor} />
            <stop offset="100%" stopColor={l.botColor} />
          </linearGradient>
        ))}
      </defs>
      {STAGES.map((s, i) => {
        const y = pad.t + rowH * i + rowH / 2;
        return (
          <g key={s.key}>
            <line x1={pad.l} x2={plotW - pad.r} y1={y} y2={y} stroke="var(--border)" strokeOpacity="0.55" />
            <text x={0} y={y + 3} className="axis">{s.label}</text>
          </g>
        );
      })}
      {links.map((l) => (
        <line key={l.id} x1={l.xj} y1={l.top} x2={l.xj} y2={l.bot} stroke={`url(#${l.id})`} strokeWidth={1.6} />
      ))}
      {bars.map((b, i) => (
        <rect
          key={i}
          x={b.left}
          y={b.y - b.h / 2}
          width={b.w}
          height={b.h}
          rx={capsuleRadius(b.w, b.h)}
          ry={capsuleRadius(b.w, b.h)}
          fill={colorOf(b.stage)}
        />
      ))}
      {ticks.map((h) => (
        <text key={h} x={Math.min(plotW - pad.r, Math.max(pad.l, x(h)))} y={H - 6} textAnchor="middle" className="axis">
          {fmtTick(h)}
        </text>
      ))}
    </svg>
  );
}


/** Parse "YYYY-MM-DD HH:MM:SS" (or ISO) as local wall-clock ms. */
function parseLocal(s: string): number {
  const m = s.replace("T", " ").match(/(\d{4})-(\d{2})-(\d{2})[ T](\d{2}):(\d{2})(?::(\d{2}))?/);
  if (!m) return Date.parse(s);
  return new Date(+m[1], +m[2] - 1, +m[3], +m[4], +m[5], +(m[6] || 0)).getTime();
}

/** Minutes since a clock origin (default 21:00) wrapping overnight. */
function minsFromOrigin(ms: number, originHour = 21): number {
  const d = new Date(ms);
  let m = d.getHours() * 60 + d.getMinutes() + d.getSeconds() / 60 - originHour * 60;
  if (m < 0) m += 24 * 60;
  return m;
}

const STAGE_COLOR: Record<string, string> = {
  Awake: STAGES[0].color,
  AsleepREM: STAGES[1].color,
  AsleepCore: STAGES[2].color,
  AsleepDeep: STAGES[3].color,
  AsleepUnspecified: "var(--unspecified)",
  InBed: "var(--inbed)",
};

export type SleepSeg = { stage: string; start: string; end: string };
export type SleepNightBar = {
  date: string;
  asleep_min: number;
  in_bed_min?: number;
  segments?: SleepSeg[];
};

/**
 * W/M floating stage bars on a clock y-axis (21:00 → 09:00).
 * `plain` paints all asleep segments one purple (Amounts / Comparisons sleep strip).
 * 6M callers pass weekly aggregates as single InBed-style spans via segments.
 */
export function FloatingSleepBars(props: {
  nights: SleepNightBar[];
  height?: number;
  plain?: boolean;
  originHour?: number;
  spanHours?: number;
  labels?: "dow" | "day" | "month" | "none";
}) {
  const plotW = 360;
  const H = props.height ?? 300;
  const origin = props.originHour ?? 21;
  const span = (props.spanHours ?? 12) * 60;
  const pad = { l: 6, r: 46, t: 8, b: 22 };
  const nights = props.nights;
  const n = Math.max(nights.length, 1);
  const bw = (plotW - pad.l - pad.r) / n;
  const y = (mins: number) => pad.t + ((H - pad.t - pad.b) * mins) / span;
  const ticks: number[] = [];
  for (let h = 0; h <= (props.spanHours ?? 12); h += 2) ticks.push(h * 60);
  const fmtTick = (mins: number) => {
    const total = (origin * 60 + mins) % (24 * 60);
    const hh = Math.floor(total / 60);
    return `${String(hh).padStart(2, "0")}:00`;
  };
  const xLabel = (date: string) => {
    const d = new Date(date + "T12:00:00");
    if (props.labels === "none") return "";
    if (props.labels === "month") return d.toLocaleDateString([], { month: "short" });
    if (props.labels === "day") return String(d.getDate());
    return d.toLocaleDateString([], { weekday: "short" }).slice(0, 3);
  };
  const asleepish = (stage: string) =>
    stage === "AsleepREM" || stage === "AsleepCore" || stage === "AsleepDeep" || stage === "AsleepUnspecified";
  return (
    <svg viewBox={`0 0 ${plotW} ${H}`} className="chart sleep-float" role="img" aria-label="Sleep by time of night">
      {ticks.map((m) => (
        <g key={m}>
          <line x1={pad.l} x2={plotW - pad.r} y1={y(m)} y2={y(m)} stroke="var(--border)" strokeOpacity="0.45" />
          <text x={plotW - pad.r + 4} y={y(m) + 3} className="axis">{fmtTick(m)}</text>
        </g>
      ))}
      {nights.map((night, i) => {
        const segs = (night.segments ?? []).filter((s) => s.stage !== "InBed");
        const draw = props.plain
          ? segs.filter((s) => asleepish(s.stage))
          : segs.filter((s) => asleepish(s.stage) || s.stage === "Awake");
        const cx = pad.l + i * bw;
        const barW = Math.max(3, Math.min(bw * 0.86, bw * 0.55));
        const x0 = cx + (bw - barW) / 2;
        return (
          <g key={night.date + i}>
            {draw.map((s, j) => {
              const a = minsFromOrigin(parseLocal(s.start), origin);
              const b = minsFromOrigin(parseLocal(s.end), origin);
              const top = Math.min(a, b);
              const bot = Math.max(a, b);
              if (bot < 0 || top > span) return null;
              const y1 = y(Math.max(0, top));
              const y2 = y(Math.min(span, bot));
              const hSeg = weekSegmentHeight(y2 - y1);
              const fill = props.plain ? "var(--sleep-plain, #9b8cff)" : (STAGE_COLOR[s.stage] ?? "var(--core)");
              return <rect key={j} x={x0} y={y1} width={barW} height={hSeg} fill={fill} rx={capsuleRadius(barW, hSeg)} ry={capsuleRadius(barW, hSeg)} />;
            })}
            {props.labels !== "none" && (props.labels === "dow" || props.labels === "day" || i % Math.ceil(n / 6) === 0 || i === n - 1) && (
              <text x={cx + bw / 2} y={H - 6} textAnchor="middle" className="axis">{xLabel(night.date)}</text>
            )}
          </g>
        );
      })}
    </svg>
  );
}

/** Compact HR range bars for Comparisons (min–max per sample). */
export function VitalRangeBars(props: {
  points: { start: string; end: string; min: number; max: number }[];
  color: string;
  height?: number;
  yMin?: number;
  yMax?: number;
}) {
  const H = props.height ?? 100;
  const pad = { l: 28, r: 8, t: 10, b: 20 };
  if (!props.points.length) return null;
  const t0 = Math.min(...props.points.map((p) => parseLocal(p.start)));
  const t1 = Math.max(...props.points.map((p) => parseLocal(p.end)));
  const lo = props.yMin ?? Math.min(...props.points.map((p) => p.min)) - 5;
  const hi = props.yMax ?? Math.max(...props.points.map((p) => p.max)) + 5;
  const x = (ms: number) => pad.l + ((W - pad.l - pad.r) * (ms - t0)) / Math.max(t1 - t0, 1);
  const y = (v: number) => pad.t + (H - pad.t - pad.b) * (1 - (v - lo) / Math.max(hi - lo, 1));
  const ticks = [lo, (lo + hi) / 2, hi];
  return (
    <svg viewBox={`0 0 ${W} ${H}`} className="chart" role="img">
      {ticks.map((t) => (
        <g key={t}>
          <line x1={pad.l} x2={W - pad.r} y1={y(t)} y2={y(t)} stroke="var(--border)" />
          <text x={pad.l - 4} y={y(t) + 3} textAnchor="end" className="axis">{Math.round(t)}</text>
        </g>
      ))}
      {props.points.map((p, i) => {
        const cx = (x(parseLocal(p.start)) + x(parseLocal(p.end))) / 2;
        const y1 = y(p.max);
        const y2 = y(p.min);
        return <rect key={i} x={cx - 2} y={y1} width={4} height={Math.max(2, y2 - y1)} rx={2} fill={props.color} />;
      })}
    </svg>
  );
}

/**
 * Weekly Highlights average: purple day bars with a white mean line so the
 * average stays visible across the bars. The duration sits left of the bars.
 * Yearly comparison is not drawn here.
 */
export function AvgAsleepBars(props: { nights: { date: string; asleep_min: number }[]; avg: number; height?: number }) {
  const H = props.height ?? 248;
  const pad = { l: 2, r: 8, t: 18, b: 22 };
  const nights = props.nights.slice(-7);
  const plotL = W * 0.46;
  const plotR = W - pad.r;
  const plotT = pad.t;
  const plotB = H - pad.b;
  const maxV = Math.max(props.avg, ...nights.map((n) => n.asleep_min), 1);
  const scaleMax = maxV * 1.35;
  const y = (v: number) => plotB - ((plotB - plotT) * v) / scaleMax;
  const lineY = y(props.avg);
  const n = Math.max(nights.length, 1);
  const slot = (plotR - plotL) / n;
  const barW = Math.min(slot * 0.46, 26);
  const hrs = Math.floor(props.avg / 60);
  const mins = Math.round(props.avg % 60);
  return (
    <div className="avg-sleep" style={{ ["--line" as string]: `${(lineY / H) * 100}%` }}>
      <p className="avg-kicker">Average<br />Time Asleep</p>
      <p className="avg-value num">{hrs}<small>hr</small> {mins}<small>min</small></p>
      <div className="avg-rule" />
      <svg viewBox={`0 0 ${W} ${H}`} className="chart" role="img" aria-label="Average time asleep">
        {nights.map((d, i) => {
          const cx = plotL + slot * i + slot / 2;
          const y1 = y(d.asleep_min);
          const bh = Math.max(3, plotB - y1);
          return (
            <g key={d.date}>
              <rect x={cx - barW / 2} y={y1} width={barW} height={bh} rx={4} fill="var(--avg-bar)" />
              <text x={cx} y={H - 5} textAnchor="middle" className="axis">
                {new Date(d.date + "T12:00:00").toLocaleDateString([], { weekday: "narrow" })}
              </text>
            </g>
          );
        })}
      </svg>
    </div>
  );
}
