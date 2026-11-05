// Fitness tab: Apple Fitness "All Categories" inset list (imported data only).
import { useMemo, useState } from "react";
import { Link } from "../router";
import { LargeTitle, NavBar } from "./ui";
import { BrowseMetric, CatIcon, Chevron, DISTANCE, ENERGY, STEPS, useApi, useBrowse } from "./meta";
import { today } from "./range";
import type { Workout } from "../api";

const EXERCISE = "HKQuantityTypeIdentifierAppleExerciseTime";
const STAND = "HKCategoryTypeIdentifierAppleStandHour";
const FLIGHTS = "HKQuantityTypeIdentifierFlightsClimbed";

/** Fitness All Categories rows from imported data (no Awards / Tips / Load / Trends). */
const FIXED: { key: string; name: string; to: string; icon: string; match?: (m: BrowseMetric) => boolean }[] = [
  { key: "rings", name: "Activity Rings", to: `/activity/${today()}`, icon: "activity" },
  { key: "steps", name: "Steps", to: "/summary/steps", icon: "activity", match: (m) => m.metric === STEPS },
  { key: "sessions", name: "Sessions", to: "/fitness/sessions", icon: "activity" },
];

const EXTRA_METRICS = [DISTANCE, ENERGY, EXERCISE, STAND, FLIGHTS];

function prettyWorkout(m: string) {
  return m.replace(/^HKWorkoutActivityType/, "").replace(/([a-z])([A-Z])/g, "$1 $2");
}

export default function Fitness() {
  const b = useBrowse();
  const ms = b.data?.metrics ?? [];
  const workouts = ms
    .filter((m) => m.metric.startsWith("HKWorkoutActivityType"))
    .sort((a, c) => a.name.localeCompare(c.name));
  const mind = ms.filter((m) => m.category === "mindfulness");
  const extras = EXTRA_METRICS.map((id) => ms.find((m) => m.metric === id)).filter(Boolean) as BrowseMetric[];

  return (
    <div className="page">
      <LargeTitle title="All Categories" avatar={false} />
      <p className="group-title">Fitness</p>
      <div className="group browse-cats">
        {FIXED.map((row) => {
          if (row.match && !ms.some(row.match)) return null;
          return (
            <Link key={row.key} to={row.to} className="row">
              <span className="row-icon" style={{ color: "var(--c-activity)" }}><CatIcon k={row.icon} /></span>
              <span className="row-label"><b>{row.name}</b></span>
              <Chevron />
            </Link>
          );
        })}
        {extras.map((m) => (
          <Link key={m.metric} to={`/health/metric/${encodeURIComponent(m.metric)}`} className="row">
            <span className="row-icon" style={{ color: "var(--c-activity)" }}><CatIcon k="activity" /></span>
            <span className="row-label"><b>{m.name}</b></span>
            <Chevron />
          </Link>
        ))}
        {workouts.map((m) => (
          <Link key={m.metric} to={`/health/metric/${encodeURIComponent(m.metric)}`} className="row">
            <span className="row-icon" style={{ color: "var(--c-activity)" }}><CatIcon k="activity" /></span>
            <span className="row-label"><b>{prettyWorkout(m.metric)}</b></span>
            <Chevron />
          </Link>
        ))}
        {mind.map((m) => (
          <Link key={m.metric} to={`/health/metric/${encodeURIComponent(m.metric)}`} className="row">
            <span className="row-icon" style={{ color: "var(--c-mind)" }}><CatIcon k="mindfulness" /></span>
            <span className="row-label"><b>{m.name}</b></span>
            <Chevron />
          </Link>
        ))}
      </div>
      {b.data && !ms.length && <p className="muted">No data yet. <Link to="/settings/import">Import health data</Link>.</p>}
    </div>
  );
}

function chipLabel(atype: string) {
  const p = prettyWorkout(atype);
  if (/Walk/i.test(p)) return "Walking";
  if (/Run/i.test(p)) return "Running";
  if (/Swim/i.test(p)) return "Swimming";
  if (/Strength|Traditional/i.test(p)) return "Strength";
  if (/Yoga|Mind|Cool|Flexibility/i.test(p)) return "Mind & Body";
  return "Workouts";
}

function sessionValue(w: Workout): string {
  if (w.distance_m != null && w.distance_m > 0) {
    const mi = w.distance_m / 1609.344;
    return `${mi.toFixed(2)}MI`;
  }
  if (w.active_kcal != null) return `${Math.round(w.active_kcal)}CAL`;
  if (w.duration_min != null) {
    const m = Math.round(w.duration_min);
    const h = Math.floor(m / 60);
    const mm = m % 60;
    return h ? `${h}:${String(mm).padStart(2, "0")}` : `0:${String(mm).padStart(2, "0")}`;
  }
  return "";
}

function monthHeading(iso: string) {
  const d = new Date((iso.slice(0, 10)) + "T12:00:00");
  return d.toLocaleDateString([], { month: "long", year: "numeric" });
}

/** Sessions: chips All/Workouts/types, month headings, cards with icon, lime value, date — no edit. */
export function FitnessSessions() {
  const wlist = useApi<{ workouts: Workout[] }>("/workouts?limit=100");
  const workouts = wlist.data?.workouts ?? [];
  const chips = useMemo(() => {
    const set = new Set<string>();
    for (const w of workouts) set.add(chipLabel(w.activity_type));
    return ["All", ...Array.from(set).sort()];
  }, [workouts]);
  const [chip, setChip] = useState("All");
  const filtered = workouts.filter((w) => chip === "All" || chipLabel(w.activity_type) === chip);
  const groups: { month: string; items: Workout[] }[] = [];
  for (const w of filtered) {
    const key = monthHeading(w.local_start ?? w.start_utc);
    const last = groups[groups.length - 1];
    if (last && last.month === key) last.items.push(w);
    else groups.push({ month: key, items: [w] });
  }
  return (
    <div className="page sessions-page">
      <NavBar to="/fitness" title="Sessions" />
      <div className="chip-row" role="tablist">
        {chips.map((c) => (
          <button key={c} type="button" role="tab" aria-selected={chip === c} className={chip === c ? "chip on" : "chip"} onClick={() => setChip(c)}>{c}</button>
        ))}
      </div>
      {groups.map((g) => (
        <div key={g.month} className="session-month">
          <p className="group-title">{g.month}</p>
          {g.items.map((w) => (
            <Link key={w.id ?? w.external_id ?? w.start_utc} to={`/workouts/${w.id}`} className="session-card">
              <span className="session-icon" aria-hidden><CatIcon k="activity" size={22} /></span>
              <div className="session-body">
                <b>{prettyWorkout(w.activity_type)}</b>
                <span className="session-val num">{sessionValue(w)}</span>
                <span className="muted small session-date">{(w.local_start ?? w.start_utc).slice(0, 10)}</span>
              </div>
            </Link>
          ))}
        </div>
      ))}
      {wlist.data && !filtered.length && <p className="muted pad">No sessions yet.</p>}
    </div>
  );
}
