// Fitness tab: Apple Fitness "All Categories" inset list (imported data only).
import { Link } from "../router";
import { LargeTitle, NavBar } from "./ui";
import { BrowseMetric, CatIcon, Chevron, DISTANCE, ENERGY, STEPS, useApi, useBrowse, when } from "./meta";
import { today } from "./range";

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

/** Sessions list — no Apple Sessions screenshot yet; plain inset list. */
export function FitnessSessions() {
  const wlist = useApi<{ workouts: import("../api").Workout[] }>("/workouts?limit=50");
  return (
    <div className="page">
      <NavBar to="/fitness" title="Sessions" />
      <div className="group">
        {(wlist.data?.workouts ?? []).map((w) => (
          <Link key={w.id} to={`/workouts/${w.id}`} className="row">
            <span className="row-label">
              <b>{prettyWorkout(w.activity_type)}</b>
              <small className="muted">{when(w.local_start ?? w.start_utc)}</small>
            </span>
            <span className="row-value num">{w.duration_min != null ? `${Math.round(w.duration_min)} min` : ""}</span>
            <Chevron />
          </Link>
        ))}
        {wlist.data && !wlist.data.workouts.length && <p className="muted pad">No sessions yet.</p>}
      </div>
    </div>
  );
}
