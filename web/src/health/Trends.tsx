// Trends: last 90 days vs last 365, per metric, with a detail chart.
import { useMemo } from "react";
import { SeriesData } from "../api";
import { Link } from "../router";
import { conv, useHealth } from "./ctx";
import { DISTANCE, STEPS, useApi } from "./meta";
import { addDays, today } from "./range";
import { NavBar, LargeTitle } from "./ui";
import type { RingDay } from "./Summary";

type Def = { key: string; label: string; color: string; unit: string; desc: string; metric?: string; ring?: keyof RingDay; fmt?: (v: number) => string };
const DEFS: Def[] = [
  { key: "move", label: "Move", color: "var(--move)", unit: "CAL/DAY", desc: "Average active calories burned per day.", ring: "move_kcal" },
  { key: "exercise", label: "Exercise", color: "var(--exercise)", unit: "MIN/DAY", desc: "Average minutes of brisk activity per day.", ring: "exercise_min" },
  { key: "stand", label: "Stand Hours", color: "var(--stand)", unit: "HRS/DAY", desc: "Average hours with at least a minute of standing.", ring: "stand_hours" },
  { key: "steps", label: "Steps", color: "var(--steps)", unit: "STEPS/DAY", desc: "Average steps taken per day.", metric: STEPS },
  { key: "distance", label: "Distance", color: "var(--distance)", unit: "MI/DAY", desc: "Average walking and running distance per day.", metric: DISTANCE, fmt: (v) => v.toFixed(1) },
  { key: "speed", label: "Walking Pace", color: "var(--distance)", unit: "MPH", desc: "Average walking speed on flat ground.", metric: "HKQuantityTypeIdentifierWalkingSpeed", fmt: (v) => v.toFixed(1) },
  { key: "vo2", label: "Cardio Fitness", color: "var(--c-heart)", unit: "VO2 MAX", desc: "Estimated VO2 max from outdoor walks and runs.", metric: "HKQuantityTypeIdentifierVO2Max", fmt: (v) => v.toFixed(1) },
];

type T = Def & { pts: { date: string; value: number }[]; a90: number; a365: number; enough: boolean; up: boolean };

function useTrends(): T[] | null {
  const { units } = useHealth();
  const end = today();
  const rings = useApi<{ days: RingDay[] }>(`/rings?start=${addDays(end, -364)}&end=${end}`);
  const s: Record<string, SeriesData | null> = {};
  for (const d of DEFS.filter((d) => d.metric)) {
    // eslint-disable-next-line react-hooks/rules-of-hooks
    s[d.key] = useApi<SeriesData>(`/series?metric=${encodeURIComponent(d.metric!)}&bucket=day&days=365&end=${end}`).data ?? null;
  }
  return useMemo(() => {
    if (!rings.data) return null;
    return DEFS.map((d) => {
      let pts: { date: string; value: number }[];
      if (d.ring) pts = rings.data!.days.filter((x) => x.move_kcal > 0 || x.exercise_min > 0 || x.stand_hours > 0).map((x) => ({ date: x.date, value: Number(x[d.ring!]) }));
      else pts = (s[d.key]?.points ?? []).map((p) => ({ date: p.date, value: conv(p.value, s[d.key]?.unit ?? null, units)[0] }));
      const cut = addDays(end, -89);
      const avg = (xs: typeof pts) => (xs.length ? xs.reduce((a, x) => a + x.value, 0) / xs.length : 0);
      const a90 = avg(pts.filter((p) => p.date >= cut)), a365 = avg(pts);
      const enough = pts.length >= 180 && pts.some((p) => p.date >= cut);
      const unit = d.key === "distance" && units.distance === "km" ? "KM/DAY" : d.key === "speed" && units.distance === "km" ? "KM/HR" : d.unit;
      return { ...d, unit, pts, a90, a365, enough, up: a90 >= a365 };
    });
  }, [rings.data, ...Object.values(s), units]); // eslint-disable-line react-hooks/exhaustive-deps
}

const Arrow = ({ up, color }: { up: boolean; color: string }) => (
  <span className="tarrow" style={{ background: color }}>
    <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden style={{ transform: up ? "" : "rotate(180deg)" }}><path d="M8 13V3M3.5 7.5 8 3l4.5 4.5" fill="none" stroke="#000" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" /></svg>
  </span>
);

function headline(name: string, ts: T[]) {
  const ok = ts.filter((t) => t.enough);
  if (!ok.length) return `Keep wearing your watch, ${name}. Trends appear after about six months of data.`;
  const down = ok.filter((t) => !t.up);
  if (!down.length) return `All of your trends are looking good, ${name}. Incredible! Don't let up.`;
  if (down.length === ok.length) return `Your trends are dipping, ${name}. A little more each day turns the arrows back up.`;
  if (down.length * 2 > ok.length) return `A few of your trends are slipping, ${name}. Small, steady days will turn them around.`;
  return `Most of your trends are heading the right way, ${name}. Give ${down.map((d) => d.label.toLowerCase()).join(" and ")} a nudge.`;
}

export default function Trends() {
  const { name } = useHealth();
  const ts = useTrends();
  const fmt = (t: T, v: number) => (t.fmt ? t.fmt(v) : Math.round(v).toLocaleString());
  return (
    <div className="page trends">
      <NavBar to="/" />
      <LargeTitle title="Trends" avatar={false} />
      {!ts ? <p className="muted">Loading…</p> : (
        <>
          <p className="trend-head">{headline(name, ts)}</p>
          <div className="tiles">
            {ts.filter((t) => t.enough).map((t) => (
              <Link key={t.key} to={`/summary/trends/${t.key}`} className="tile trendtile">
                <Arrow up={t.up} color={t.up ? t.color : "var(--muted)"} />
                <b>{t.label}</b>
                <p className="big num" style={{ color: t.up ? t.color : "var(--muted)" }}>{fmt(t, t.a90)}<small>{t.unit}</small></p>
              </Link>
            ))}
          </div>
          {ts.some((t) => !t.enough) && <h2>Needs More Data</h2>}
          {ts.filter((t) => !t.enough).map((t) => (
            <div key={t.key} className="card needsdata">
              <span className="tarrow dash"><svg width="14" height="14" viewBox="0 0 14 14" aria-hidden><path d="M3 7h8" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" /></svg></span>
              <div><b>{t.label}</b><p className="big num muted">-/-<small>{t.unit.split("/")[0]}</small></p><p className="muted small">{t.desc}</p></div>
            </div>
          ))}
          <p className="muted small foot">Trends compares your last 90 days of activity to the last 365. If you're doing the same or better, your arrow will be up; if not, it will be down.</p>
        </>
      )}
    </div>
  );
}

export function TrendDetail({ k }: { k: string }) {
  const ts = useTrends();
  const t = ts?.find((x) => x.key === k);
  const W = 340, H = 200;
  const chart = useMemo(() => {
    if (!t || !t.pts.length) return null;
    const max = Math.max(...t.pts.map((p) => p.value), t.a90, t.a365) * 1.1 || 1;
    const start = addDays(today(), -364);
    const x = (d: string) => ((new Date(d).getTime() - new Date(start).getTime()) / 864e5 / 365) * W;
    const y = (v: number) => H - (v / max) * H;
    return { x, y, max };
  }, [t]);
  return (
    <div className="page">
      <NavBar to="/summary/trends" title={t?.label} />
      {t && chart && (
        <section className="card">
          <p className="eyebrow">90-DAY AVERAGE</p>
          <p className="big xl num" style={{ color: t.color }}>{t.fmt ? t.fmt(t.a90) : Math.round(t.a90).toLocaleString()}<small>{t.unit}</small></p>
          <p className="muted small">365-day average: <span className="num">{t.fmt ? t.fmt(t.a365) : Math.round(t.a365).toLocaleString()}</span> {t.unit}</p>
          <svg viewBox={`0 0 ${W} ${H + 16}`} className="trendchart" aria-label="Daily values with averages">
            {t.pts.map((p) => <rect key={p.date} x={chart.x(p.date)} width="0.8" y={chart.y(p.value)} height={H - chart.y(p.value)} fill="var(--muted)" opacity=".5" />)}
            <line x1="0" x2={W} y1={chart.y(t.a365)} y2={chart.y(t.a365)} stroke="var(--text)" strokeWidth="2" strokeDasharray="4 4" />
            <line x1={W * (275 / 365)} x2={W} y1={chart.y(t.a90)} y2={chart.y(t.a90)} stroke={t.color} strokeWidth="4" strokeLinecap="round" />
            <text x="0" y={H + 14} className="axis">365 days ago</text>
            <text x={W} y={H + 14} textAnchor="end" className="axis">Today</text>
          </svg>
          <p className="legend"><i style={{ background: t.color }} /> Last 90 days <i style={{ background: "var(--text)" }} /> Last 365 days</p>
          <p className="muted small">{t.desc}</p>
        </section>
      )}
      {t && !t.enough && <p className="muted">Not enough data for a trend yet.</p>}
    </div>
  );
}
