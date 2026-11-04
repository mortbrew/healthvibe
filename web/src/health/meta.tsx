// Browse categories, colors, icons (original SVGs) and value formatting.
import { api } from "../api";
import { useEffect, useState } from "react";

export type BrowseMetric = {
  metric: string;
  name: string;
  unit: string | null;
  count: number;
  first: string | null;
  last: string | null;
  agg: "sum" | "avg" | "count";
  category: string;
  latest_value: string | null;
  latest_at: string | null;
};

export const STEPS = "HKQuantityTypeIdentifierStepCount";
export const DISTANCE = "HKQuantityTypeIdentifierDistanceWalkingRunning";
export const ENERGY = "HKQuantityTypeIdentifierActiveEnergyBurned";
export const RESTING = "HKQuantityTypeIdentifierRestingHeartRate";
export const SLEEP = "HKCategoryTypeIdentifierSleepAnalysis";
export const WEIGHT = "HKQuantityTypeIdentifierBodyMass";
export const HRV = "HKQuantityTypeIdentifierHeartRateVariabilitySDNN";
export const DEFAULT_PINNED = [STEPS, ENERGY, RESTING, SLEEP, WEIGHT];

export const CATEGORIES: { key: string; name: string; color: string }[] = [
  { key: "activity", name: "Activity", color: "var(--c-activity)" },
  { key: "body", name: "Body Measurements", color: "var(--c-body)" },
  { key: "heart", name: "Heart", color: "var(--c-heart)" },
  { key: "mindfulness", name: "Mental Wellbeing", color: "var(--c-mind)" },
  { key: "mobility", name: "Mobility", color: "var(--c-mobility)" },
  { key: "nutrition", name: "Nutrition", color: "var(--c-nutrition)" },
  { key: "respiratory", name: "Respiratory", color: "var(--c-resp)" },
  { key: "sleep", name: "Sleep", color: "var(--c-sleep)" },
  { key: "vitals", name: "Vitals", color: "var(--c-vitals)" },
  { key: "other", name: "Other Data", color: "var(--c-other)" },
];
export const cat = (k: string) => CATEGORIES.find((c) => c.key === k) ?? CATEGORIES[CATEGORIES.length - 1];

const P: Record<string, React.ReactNode> = {
  activity: <path d="M12 2c1 3.5 5 5.8 5 11a5 5 0 0 1-10 0c0-2.4 1.2-3.8 2.3-5 .2 1.7 1 2.8 2.2 3.2C11 8.8 11 5 12 2Z" />,
  heart: <path d="M12 20.5S3.5 15.3 3.5 9.2A4.4 4.4 0 0 1 12 7a4.4 4.4 0 0 1 8.5 2.2c0 6.1-8.5 11.3-8.5 11.3Z" />,
  sleep: <path d="M14.5 3.2A8.5 8.5 0 1 0 20.8 15 7 7 0 0 1 14.5 3.2Z" />,
  body: <path d="M12 2.5a2.5 2.5 0 1 1 0 5 2.5 2.5 0 0 1 0-5ZM5 8.5h14v2h-4.5v11h-2v-5h-1v5h-2v-11H5Z" />,
  mobility: <path d="M4 17l5-5 3 3 6-6M14 9h4v4" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" />,
  respiratory: <path d="M3 9h11a3 3 0 1 0-3-3M3 13h15a3 3 0 1 1-3 3M3 17h7" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" />,
  vitals: <path d="M2.5 12h4l2-5 4 10 2.5-6 1.5 1h5" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" />,
  nutrition: <path d="M12 7c-1-3 1-5 3-5 0 2-1 3.5-3 5Zm0 1c-4-2-8 0-8 5s3 9 5 9c1 0 2-.6 3-.6s2 .6 3 .6c2 0 5-4 5-9s-4-7-8-5Z" />,
  mindfulness: <path d="M12 3c2 2.5 2 6 0 9-2-3-2-6.5 0-9Zm-7 6c3 0 5.5 2 7 5-3 0-6-1.5-7-5Zm14 0c-1 3.5-4 5-7 5 1.5-3 4-5 7-5ZM4 17c4 1 12 1 16 0-1 2.5-4.5 4-8 4s-7-1.5-8-4Z" />,
  other: <path d="M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z" />,
};

export function CatIcon({ k, size = 18 }: { k: string; size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" aria-hidden style={{ color: cat(k).color, flex: "none" }}>
      {P[k] ?? P.other}
    </svg>
  );
}

const UNIT: Record<string, string> = { count: "", "count/min": "BPM", kcal: "kcal", Cal: "kcal", min: "min", "%": "%", ms: "ms" };
export const unitLabel = (m: { metric: string; unit: string | null }) => {
  if (m.metric === STEPS) return "steps";
  if (m.metric.startsWith("HKCategory")) return "";
  return m.unit ? UNIT[m.unit] ?? m.unit : "";
};

export const fmtNum = (v: number, metric = "") => {
  const isPct = metric.includes("Percentage");
  const x = isPct && v <= 1 ? v * 100 : v;
  if (Math.abs(x) >= 100 || metric === STEPS) return Math.round(x).toLocaleString();
  return x.toLocaleString(undefined, { maximumFractionDigits: Math.abs(x) >= 10 ? 1 : 2 });
};

export const hm = (min: number) => {
  const h = Math.floor(min / 60), m = Math.round(min % 60);
  return h ? `${h} hr ${m} min` : `${m} min`;
};

/** "Today", "Yesterday", weekday, or a short date; with time for today. */
export function when(ts: string | null) {
  if (!ts) return "";
  const d = new Date(ts.replace(" ", "T").slice(0, 19));
  if (isNaN(+d)) return ts.slice(0, 10);
  const days = Math.round((startOfDay(new Date()) - startOfDay(d)) / 864e5);
  if (days === 0) return d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  if (days === 1) return "Yesterday";
  if (days < 7) return d.toLocaleDateString([], { weekday: "long" });
  return d.toLocaleDateString([], { month: "short", day: "numeric", year: d.getFullYear() === new Date().getFullYear() ? undefined : "numeric" });
}
const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();

/** Fetches a path; refetches when it changes. */
export function useApi<T>(path: string | null) {
  const [state, setState] = useState<{ data?: T; error?: boolean }>({});
  useEffect(() => {
    if (!path) return;
    let live = true;
    setState({});
    api<T>(path).then((data) => live && setState({ data }), () => live && setState({ error: true }));
    return () => {
      live = false;
    };
  }, [path]);
  return state;
}

export const useBrowse = () => useApi<{ metrics: BrowseMetric[] }>("/browse");

export function Chevron() {
  return (
    <svg className="chev" width="8" height="13" viewBox="0 0 8 13" aria-hidden>
      <path d="M1.5 1.5 6.5 6.5 1.5 11.5" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}
export function ChevronCircle() {
  return (
    <span className="chevc" aria-hidden>
      <svg width="7" height="11" viewBox="0 0 8 13"><path d="M1.5 1.5 6.5 6.5 1.5 11.5" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" /></svg>
    </span>
  );
}

/** Tiny line/bars for cards. */
export function Spark({ values, color, bars }: { values: number[]; color: string; bars?: boolean }) {
  const W = 120, H = 36;
  if (!values.length) return <svg className="spark" viewBox={`0 0 ${W} ${H}`} />;
  const max = Math.max(...values, 1e-9), min = bars ? 0 : Math.min(...values);
  const y = (v: number) => H - 3 - ((v - min) / (max - min || 1)) * (H - 6);
  const bw = W / values.length;
  return (
    <svg className="spark" viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" aria-hidden>
      {bars
        ? values.map((v, i) => <rect key={i} x={i * bw + bw * 0.2} width={bw * 0.6} y={y(v)} height={H - y(v)} rx={1.5} fill={color} opacity={i === values.length - 1 ? 1 : 0.55} />)
        : <polyline fill="none" stroke={color} strokeWidth="2.2" strokeLinejoin="round" strokeLinecap="round"
            points={values.map((v, i) => `${(i / Math.max(values.length - 1, 1)) * (W - 4) + 2},${y(v)}`).join(" ")} />}
    </svg>
  );
}
