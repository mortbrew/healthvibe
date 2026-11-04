// D / W / M / 6M / Y ranges, paging back in time, and the Apple-style bar chart.
import { useEffect, useRef, useState } from "react";
import { api, SeriesData } from "../api";

export type Range = "D" | "W" | "M" | "6M" | "Y";
const DAYS: Record<Range, number> = { D: 1, W: 7, M: 30, "6M": 182, Y: 365 };

export const addDays = (d: string, n: number) => {
  const x = new Date(d + "T12:00:00");
  x.setDate(x.getDate() + n);
  return x.toISOString().slice(0, 10);
};
export const today = () => {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
};
const fmt = (d: string, o: Intl.DateTimeFormatOptions) => new Date(d + "T12:00:00").toLocaleDateString([], o);
export const dayTitle = (d: string) => (d === today() ? "Today" : d === addDays(today(), -1) ? "Yesterday" : fmt(d, { weekday: "short", month: "short", day: "numeric", year: "numeric" }));

export type Bucket = { key: string; label: string; value: number; has: boolean; title: string };
export type RangeData = { buckets: Bucket[]; start: string; end: string; summary: number | null; unit: string | null; agg: string };

/** Loads one range for a metric. `offset` pages back (0 = the range ending at `anchor`). */
export function useRange(metric: string, range: Range, anchor: string | null, offset: number) {
  const [data, setData] = useState<RangeData | null>(null);
  useEffect(() => {
    if (!anchor) return;
    let live = true;
    setData(null);
    const span = DAYS[range];
    const end = addDays(anchor, -span * offset);
    const start = addDays(end, 1 - span);
    const q = (p: string) => api<any>(p);
    (range === "D"
      ? q(`/hourly?metric=${encodeURIComponent(metric)}&date=${end}`).then((h) => {
          const vals: number[] = h.values;
          const buckets = vals.map((v, i) => ({ key: String(i), label: i % 6 === 0 ? String(i).padStart(2, "0") : "", value: v, has: v > 0, title: `${String(i).padStart(2, "0")}:00` }));
          const total = vals.reduce((a, b) => a + b, 0);
          const agg = h.agg ?? (metric.startsWith("HKCategory") ? "count" : null);
          return { buckets, start: end, end, summary: buckets.some((b) => b.has) ? total : null, unit: h.unit, agg: agg ?? "sum" };
        })
      : q(`/series?metric=${encodeURIComponent(metric)}&bucket=day&days=${span}&end=${end}`).then((s: SeriesData) => {
          const by = new Map(s.points.map((p) => [p.date, p.value]));
          const days = Array.from({ length: span }, (_, i) => addDays(start, i));
          const groups = new Map<string, string[]>();
          for (const d of days) {
            let k = d;
            if (range === "6M") {
              const wd = (new Date(d + "T12:00:00").getDay() + 6) % 7;
              k = addDays(d, -wd);
            } else if (range === "Y") k = d.slice(0, 7);
            groups.set(k, [...(groups.get(k) ?? []), d]);
          }
          const buckets: Bucket[] = [...groups.entries()].map(([k, ds], i) => {
            const vs = ds.filter((d) => by.has(d)).map((d) => by.get(d)!);
            const value = vs.length ? vs.reduce((a, b) => a + b, 0) / (range === "W" || range === "M" ? 1 : vs.length) : 0;
            const label =
              range === "W" ? fmt(k, { weekday: "narrow" }) :
              range === "M" ? (i % 7 === 0 ? fmt(k, { day: "numeric" }) : "") :
              range === "6M" ? (Number(k.slice(8, 10)) <= 7 ? fmt(k, { month: "short" }) : "") :
              fmt(k + "-01", { month: "narrow" });
            const title = range === "Y" ? fmt(k + "-01", { month: "long", year: "numeric" }) : range === "6M" ? `Week of ${fmt(k, { month: "short", day: "numeric" })}` : fmt(k, { weekday: "short", month: "short", day: "numeric" });
            return { key: k, label, value, has: vs.length > 0, title };
          });
          const vals = days.filter((d) => by.has(d)).map((d) => by.get(d)!);
          const summary = vals.length ? vals.reduce((a, b) => a + b, 0) / vals.length : null;
          return { buckets, start, end, summary, unit: s.unit, agg: s.agg };
        })
    ).then((d) => live && setData(d), () => live && setData({ buckets: [], start, end, summary: null, unit: null, agg: "sum" }));
    return () => {
      live = false;
    };
  }, [metric, range, anchor, offset]);
  return data;
}

export const rangeLabel = (r: Range, d: RangeData) => {
  if (r === "D") return dayTitle(d.end);
  const o: Intl.DateTimeFormatOptions = { month: "short", day: "numeric", year: "numeric" };
  return `${fmt(d.start, o)} – ${fmt(d.end, o)}`;
};

export function Segmented({ value, options, onChange }: { value: Range; options: Range[]; onChange: (r: Range) => void }) {
  return (
    <div className="seg" role="tablist">
      {options.map((o) => (
        <button key={o} role="tab" aria-selected={o === value} className={o === value ? "on" : ""} onClick={() => onChange(o)}>{o}</button>
      ))}
    </div>
  );
}

const nice = (max: number) => {
  if (max <= 0) return 1;
  const p = Math.pow(10, Math.floor(Math.log10(max)));
  for (const m of [1, 2, 2.5, 5, 10]) if (m * p >= max) return m * p;
  return 10 * p;
};

/** Bar chart with right-side y axis, dashed vertical gridlines, tap-to-inspect and swipe paging. */
export function RangeChart(props: {
  buckets: Bucket[];
  color: string;
  height?: number;
  selected?: number | null;
  onSelect?: (i: number | null) => void;
  onSwipe?: (dir: 1 | -1) => void;
  fmt?: (v: number) => string;
  gradient?: boolean;
  dots?: boolean;
}) {
  const W = 340, H = props.height ?? 150, R = 34, B = 18;
  const n = Math.max(props.buckets.length, 1);
  const top = nice(Math.max(0, ...props.buckets.map((b) => b.value)));
  const bw = (W - R) / n;
  const y = (v: number) => 6 + (H - B - 6) * (1 - v / top);
  const touch = useRef<number | null>(null);
  const f = props.fmt ?? ((v: number) => v.toLocaleString(undefined, { maximumFractionDigits: v < 10 ? 2 : 0 }));
  const gid = useRef("g" + Math.random().toString(36).slice(2)).current;
  return (
    <svg
      viewBox={`0 0 ${W} ${H}`}
      className="rchart"
      role="img"
      onTouchStart={(e) => (touch.current = e.touches[0].clientX)}
      onTouchEnd={(e) => {
        if (touch.current === null) return;
        const dx = e.changedTouches[0].clientX - touch.current;
        touch.current = null;
        if (Math.abs(dx) > 50) props.onSwipe?.(dx > 0 ? 1 : -1);
      }}
    >
      {props.gradient && (
        <>
          <defs>
            <linearGradient id={gid} x1="0" x2="0" y1="0" y2="1">
              <stop offset="0" stopColor={props.color} stopOpacity="0.16" />
              <stop offset="1" stopColor={props.color} stopOpacity="0" />
            </linearGradient>
          </defs>
          <rect x={0} y={0} width={W - R} height={H - B} fill={`url(#${gid})`} />
        </>
      )}
      {[0, 0.5, 1].map((t) => (
        <g key={t}>
          <line x1={0} x2={W - R} y1={y(top * t)} y2={y(top * t)} className={props.dots && t === 0 ? "gline dots" : "gline"} style={props.dots && t === 0 ? { stroke: props.color } : undefined} />
          <text x={W - R + 4} y={y(top * t) + 4} className="axis">{f(top * t)}</text>
        </g>
      ))}
      {props.buckets.map((b, i) =>
        b.label ? (
          <g key={"l" + b.key}>
            <line x1={i * bw} x2={i * bw} y1={0} y2={H - B + 4} className="gline dash" />
            <text x={i * bw + 3} y={H - 4} className="axis">{b.label}</text>
          </g>
        ) : null,
      )}
      <line x1={W - R} x2={W - R} y1={0} y2={H - B + 4} className="gline dash" />
      {props.buckets.map((b, i) => (
        <rect
          key={b.key}
          x={i * bw + bw * 0.18}
          width={Math.max(bw * 0.64, 1)}
          y={y(b.value)}
          height={Math.max(0, H - B - y(b.value))}
          rx={Math.min(3, bw * 0.3)}
          fill={props.color}
          opacity={props.selected == null || props.selected === i ? 1 : 0.35}
        />
      ))}
      {props.onSelect && props.buckets.map((b, i) => (
        <rect key={"h" + b.key} x={i * bw} width={bw} y={0} height={H} fill="transparent"
          onClick={() => props.onSelect!(props.selected === i ? null : i)} style={{ cursor: "pointer" }}>
          <title>{b.title}</title>
        </rect>
      ))}
    </svg>
  );
}
