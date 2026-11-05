// Summary (Fitness-style), Activity day view + calendar, Steps & Distance details.
import { useEffect, useMemo, useRef, useState } from "react";
import { Activity, api, SeriesData } from "../api";
import { Link, navigate } from "../router";
import { useHealth, conv } from "./ctx";
import { BrowseMetric, cat, CatIcon, ChevronCircle, DEFAULT_PINNED, DISTANCE, ENERGY, fmtNum, Spark, STEPS, unitLabel, useApi, useBrowse, when } from "./meta";
import { addDays, rangeLabel, Range, RangeChart, Segmented, today, useRange, dayTitle } from "./range";
import { Badge, FitRings, ringsOf } from "./rings";
import { CalIcon, LargeTitle, NavBar, RoundBtn, Sheet, XIcon } from "./ui";
import type { AwardOut } from "./Awards";

export const EXERCISE = "HKQuantityTypeIdentifierAppleExerciseTime";
export const STAND = "HKCategoryTypeIdentifierAppleStandHour";
export type RingDay = { date: string; move_kcal: number; move_goal: number; exercise_min: number; exercise_goal: number; stand_hours: number; stand_goal: number; workout: boolean };

const n0 = (v: number) => Math.round(v).toLocaleString();
const longDay = (d: string) => new Date(d + "T12:00:00").toLocaleDateString([], { month: "short", day: "numeric", year: "numeric" });
const hourly = (m: string, d?: string | null) => `/hourly?metric=${encodeURIComponent(m)}${d ? `&date=${d}` : ""}`;
const prettyWorkout = (m: string) => m.replace(/^HKWorkoutActivityType/, "").replace(/([a-z])([A-Z])/g, "$1 $2");

/** 24 hourly bars with 00/06/12/18 axis, used on small cards. */
function MiniHours({ values, color, height = 70 }: { values: number[]; color: string; height?: number }) {
  const W = 160, H = height, max = Math.max(...values, 1e-9);
  const bw = W / 24;
  return (
    <svg viewBox={`0 0 ${W} ${H + 14}`} className="minihours" aria-hidden>
      <line x1="0" x2={W} y1={H} y2={H} className="gline" />
      {[0, 6, 12, 18].map((h) => (
        <g key={h}>
          <line x1={h * bw} x2={h * bw} y1="0" y2={H} className="gline dash" />
          <text x={h * bw + 2} y={H + 12} className="axis">{String(h).padStart(2, "0")}</text>
        </g>
      ))}
      {values.map((v, i) => v > 0 && <rect key={i} x={i * bw + bw * 0.2} width={bw * 0.6} y={H - (v / max) * (H - 4)} height={(v / max) * (H - 4)} rx="1.5" fill={color} />)}
    </svg>
  );
}

function Tile({ title, to, children }: { title: string; to: string; children: React.ReactNode }) {
  return (
    <Link to={to} className="tile">
      <div className="tile-head"><b>{title}</b><ChevronCircle /></div>
      {children}
    </Link>
  );
}

export default function Summary() {
  const { user, units } = useHealth();
  const act = useApi<Activity>("/dashboard/activity");
  const browse = useBrowse();
  const steps = useApi<{ date: string | null; values: number[] }>(hourly(STEPS));
  const dist = useApi<{ date: string | null; values: number[]; unit: string | null }>(hourly(DISTANCE));
  const awards = useApi<{ enabled: boolean; awards: AwardOut[] }>("/awards");
  const wlist = useApi<{ workouts: import("../api").Workout[] }>("/workouts?limit=5");
  const r = act.data?.rings;
  const metrics = browse.data?.metrics ?? [];
  const workouts = metrics.filter((m) => m.metric.startsWith("HKWorkoutActivityType"));
  const lastW = [...workouts].sort((a, b) => (b.latest_at ?? "").localeCompare(a.latest_at ?? ""))[0];
  const totalW = workouts.reduce((a, m) => a + m.count, 0);
  const latestAward = (awards.data?.awards ?? []).filter((a) => a.earned).sort((a, b) => b.earned!.latest.localeCompare(a.earned!.latest))[0];
  const sum = (v?: number[]) => (v ?? []).reduce((a, b) => a + b, 0);
  const [dv, du] = conv(sum(dist.data?.values), dist.data?.unit ?? "mi", units);

  return (
    <div className="page summary">
      <LargeTitle title="Summary" eyebrow={new Date().toLocaleDateString([], { weekday: "long", month: "short", day: "numeric" }).toUpperCase()} />
      <Link to={`/activity/${r?.date ?? today()}`} className="card ringcard">
        <div className="tile-head"><b>Activity Rings</b><ChevronCircle /></div>
        <div className="ringrow">
          <FitRings v={r ? ringsOf(r) : null} size={150} />
          <dl className="ringstats">
            <div><dt>Move</dt><dd style={{ color: "var(--move)" }}><span className="num">{r ? n0(r.move_kcal) : "--"}/{r ? n0(r.move_goal) : "--"}</span><small>CAL</small></dd></div>
            <div><dt>Exercise</dt><dd style={{ color: "var(--exercise)" }}><span className="num">{r ? n0(r.exercise_min) : "--"}/{r ? n0(r.exercise_goal) : "--"}</span><small>MIN</small></dd></div>
            <div><dt>Stand</dt><dd style={{ color: "var(--stand)" }}><span className="num">{r ? n0(r.stand_hours) : "--"}/{r ? n0(r.stand_goal) : "--"}</span><small>HRS</small></dd></div>
          </dl>
        </div>
        {r && r.date !== today() && <p className="muted small">Latest data: {dayTitle(r.date)}</p>}
      </Link>

      <div className="tiles">
        <Tile title="Steps" to="/summary/steps">
          <p className="tile-sub">{steps.data?.date ? dayTitle(steps.data.date) : "No data"}</p>
          <p className="big num" style={{ color: "var(--steps)" }}>{steps.data ? n0(sum(steps.data.values)) : "--"}</p>
          <MiniHours values={steps.data?.values ?? Array(24).fill(0)} color="var(--steps)" />
        </Tile>
        <Tile title="Step Distance" to="/summary/distance">
          <p className="tile-sub">{dist.data?.date ? dayTitle(dist.data.date) : "No data"}</p>
          <p className="big num" style={{ color: "var(--distance)" }}>{dist.data ? dv.toFixed(2) : "--"}<small>{du}</small></p>
          <MiniHours values={dist.data?.values ?? Array(24).fill(0)} color="var(--distance)" />
        </Tile>
        <Tile title="Sessions" to={wlist.data?.workouts?.[0] ? `/workouts/${wlist.data.workouts[0].id}` : (lastW ? `/browse/metric/${lastW.metric}` : "/browse/activity")}>
          {wlist.data?.workouts?.[0] ? (
            <>
              <div className="session"><span className="wicon"><WorkoutIcon /></span><span>{prettyWorkout(wlist.data.workouts[0].activity_type)}</span></div>
              <p className="big num" style={{ color: "var(--exercise)" }}>{Math.round(wlist.data.workouts[0].duration_min ?? 0)}<small>MIN</small></p>
              <p className="tile-sub">{[
                wlist.data.workouts[0].avg_hr != null ? `${Math.round(wlist.data.workouts[0].avg_hr)} bpm avg` : null,
                wlist.data.workouts[0].active_kcal != null ? `${Math.round(wlist.data.workouts[0].active_kcal)} kcal` : null,
                wlist.data.workouts[0].distance_m != null ? `${(wlist.data.workouts[0].distance_m / 1000).toFixed(2)} km` : null,
              ].filter(Boolean).join(" · ") || when(wlist.data.workouts[0].local_start ?? wlist.data.workouts[0].start_utc)}</p>
              <p className="muted small">{(wlist.data.workouts.length >= 5 ? `${totalW.toLocaleString()}+` : totalW.toLocaleString())} sessions recorded</p>
            </>
          ) : lastW ? (
            <>
              <div className="session"><span className="wicon"><WorkoutIcon /></span><span>{prettyWorkout(lastW.metric)}</span></div>
              <p className="big num" style={{ color: "var(--exercise)" }}>{Math.round(Number(lastW.latest_value ?? 0))}<small>MIN</small></p>
              <p className="tile-sub">{when(lastW.latest_at)}</p>
              <p className="muted small">{totalW.toLocaleString()} sessions recorded</p>
            </>
          ) : <p className="muted">No workouts yet.</p>}
        </Tile>
        <Tile title="Awards" to="/awards">
          {awards.data && !awards.data.enabled ? (
            <p className="muted small">Turn on Awards in your account to earn badges from your history.</p>
          ) : latestAward ? (
            <div className="award-tile">
              <Badge color={latestAward.color} shape={latestAward.shape} mark={latestAward.mark} size={78} />
              <b>{latestAward.name}</b>
              <span className="muted small">{longDay(latestAward.earned!.latest)}</span>
            </div>
          ) : <p className="muted small">Close your rings to earn awards.</p>}
        </Tile>
      </div>

      <Pinned metrics={metrics} />
      <Link to="/browse" className="card linkrow">Show All Health Data <ChevronCircle /></Link>
      <Highlights />
      {!user.awards_enabled && null}
    </div>
  );
}

export function WorkoutIcon() {
  return <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden><circle cx="14" cy="4" r="2.4" /><path d="M10.5 7.5 7 10l1 1.6 2.6-1.6 1 3.5-3.6 4.2 1.5 1.3 4-4.4 1.5 2.4v4h2v-4.6l-2-3.4.8-3.2 1.4 1.8H20v-2h-2.8l-2.4-3a2 2 0 0 0-1.6-.8Z" /></svg>;
}

/* ---------- Pinned ---------- */

function Pinned({ metrics }: { metrics: BrowseMetric[] }) {
  const [pins, setPins] = useState<string[] | null>(null);
  const [edit, setEdit] = useState(false);
  useEffect(() => {
    api<{ pinned: string[] }>("/pinned").then((r) => setPins(r.pinned), () => setPins([]));
  }, []);
  const have = new Set(metrics.map((m) => m.metric));
  const list = pins === null ? [] : pins.length ? pins : DEFAULT_PINNED.filter((p) => have.has(p));
  const save = async (next: string[]) => {
    setPins(next);
    await api("/pinned", { method: "PUT", body: { pinned: next } }).catch(() => {});
  };
  return (
    <section className="pinned">
      <div className="sec-head"><h2>Pinned</h2><button className="link" onClick={() => setEdit(true)}>Edit</button></div>
      <div className="pingrid">
        {list.map((m) => {
          const bm = metrics.find((x) => x.metric === m);
          return bm ? <PinCard key={m} m={bm} /> : null;
        })}
      </div>
      {pins !== null && !list.length && <p className="muted">Pin metrics from Browse to see them here.</p>}
      {edit && (
        <Sheet title="Edit Pinned" onClose={() => setEdit(false)}>
          <p className="muted small">Pinned metrics appear on Summary. {list.length}/30 pinned.</p>
          <div className="group">
            {metrics.map((m) => {
              const on = list.includes(m.metric);
              return (
                <button key={m.metric} className="row" onClick={() => save(on ? list.filter((x) => x !== m.metric) : [...list, m.metric].slice(0, 30))}>
                  <span className="row-icon"><CatIcon k={m.category} /></span>
                  <span className="row-label">{m.name}</span>
                  <span className={on ? "pinmark on" : "pinmark"} aria-label={on ? "Unpin" : "Pin"}>{on ? "★" : "☆"}</span>
                </button>
              );
            })}
          </div>
        </Sheet>
      )}
    </section>
  );
}

function PinCard({ m }: { m: BrowseMetric }) {
  const { units } = useHealth();
  const s = useApi<SeriesData>(`/series?metric=${encodeURIComponent(m.metric)}&bucket=day&days=14${m.last ? `&end=${m.last.slice(0, 10)}` : ""}`);
  const pts = s.data?.points ?? [];
  const last = pts[pts.length - 1];
  const c = cat(m.category);
  const [v, u] = last ? conv(last.value, m.unit, units) : [0, ""];
  const label = m.metric.startsWith("HKCategory") ? (m.metric.includes("Sleep") ? "" : "") : m.metric === STEPS ? "STEPS" : u;
  return (
    <Link to={`/browse/metric/${m.metric}`} className="card pincard">
      <div className="pin-head" style={{ color: c.color }}>
        <CatIcon k={m.category} /><b>{m.name}</b>
        <span className="muted small pin-when">{when(m.latest_at)} <ChevronCircle /></span>
      </div>
      <div className="pin-body">
        <p className="big num">{last ? (m.metric.includes("Sleep") ? `${Math.floor(v / 60)}h ${Math.round(v % 60)}m` : fmtNum(v, m.metric)) : "--"}<small>{last && !m.metric.includes("Sleep") ? label || unitLabel(m) : ""}</small></p>
        <Spark values={pts.map((p) => p.value)} color={c.color} bars={m.agg === "sum"} />
      </div>
    </Link>
  );
}

/* ---------- Highlights ---------- */

function useWeekCompare(metric: string) {
  const s = useApi<SeriesData>(`/series?metric=${encodeURIComponent(metric)}&bucket=day&days=56`);
  return useMemo(() => {
    const p = s.data?.points ?? [];
    if (p.length < 8) return null;
    const end = p[p.length - 1].date;
    const inRange = (a: number, b: number) => p.filter((x) => x.date > addDays(end, -a) && x.date <= addDays(end, -b));
    const avg = (xs: { value: number }[]) => (xs.length ? xs.reduce((a, x) => a + x.value, 0) / xs.length : 0);
    return { now: avg(inRange(7, 0)), before: avg(inRange(56, 7)), unit: s.data?.unit ?? "" };
  }, [s.data]);
}

function Highlight({ metric, name, color, unit }: { metric: string; name: string; color: string; unit: string }) {
  const c = useWeekCompare(metric);
  if (!c || !c.before) return null;
  const up = c.now >= c.before;
  const pct = Math.round(Math.abs(c.now / c.before - 1) * 100);
  const max = Math.max(c.now, c.before);
  return (
    <Link to={`/browse/metric/${metric}`} className="card highlight">
      <p className="hl-cat" style={{ color }}><CatIcon k="activity" /> {name}</p>
      <p className="hl-text">
        {pct < 3 ? `Your ${name.toLowerCase()} this week is about the same as your recent average.` :
          up ? `You're averaging ${pct}% more ${name.toLowerCase()} this week than over the previous 7 weeks.` :
          `You're averaging ${pct}% fewer ${name.toLowerCase()} this week than over the previous 7 weeks.`}
      </p>
      <div className="hl-bars">
        {[["Last 7 days", c.now, color], ["Prior 7 weeks", c.before, "var(--muted)"]].map(([l, v, col]) => (
          <div key={l as string}>
            <span className="num" style={{ color: col as string }}><b>{n0(v as number)}</b> <small>{unit}/DAY</small></span>
            <i style={{ width: `${((v as number) / max) * 100}%`, background: col as string }} />
            <small className="muted">{l}</small>
          </div>
        ))}
      </div>
    </Link>
  );
}

function Highlights() {
  return (
    <section className="highlights">
      <div className="sec-head"><h2>Highlights</h2><Link to="/summary/trends" className="link">Trends</Link></div>
      <div className="hlgrid">
        <Highlight metric={STEPS} name="Steps" color="var(--steps)" unit="STEPS" />
        <Highlight metric={ENERGY} name="Active Energy" color="var(--move)" unit="CAL" />
      </div>
      <Link to="/summary/trends" className="card linkrow">See your Trends <ChevronCircle /></Link>
    </section>
  );
}

/* ---------- Steps / Distance detail ---------- */

const RANGES: Range[] = ["D", "W", "M", "Y"];

function MetricChartCard({ metric, title, color, range, anchor, offset, unitFix }: { metric: string; title: string; color: string; range: Range; anchor: string | null; offset: number; unitFix?: string }) {
  const { units } = useHealth();
  const d = useRange(metric, range, anchor, offset);
  const [sel, setSel] = useState<number | null>(null);
  useEffect(() => setSel(null), [range, offset]);
  const cv = (v: number) => conv(v, d?.unit ?? null, units);
  const b = sel !== null ? d?.buckets[sel] : null;
  const val = b ? b.value : d?.summary;
  return (
    <section className="card">
      <p className="eyebrow">{b ? b.title.toUpperCase() : range === "D" ? "TOTAL" : "DAILY AVERAGE"}</p>
      <p className="big num" style={{ color }}>{val == null ? "--" : metric === STEPS ? n0(val) : cv(val)[0].toFixed(2)}<small>{metric === STEPS ? "STEPS" : unitFix ?? cv(1)[1]}</small></p>
      <p className="muted small">{title} · {d ? rangeLabel(range, d) : "…"}</p>
      <RangeChart buckets={d?.buckets ?? []} color={color} selected={sel} onSelect={setSel} />
    </section>
  );
}

function useAnchor(metric: string) {
  const b = useBrowse();
  const m = b.data?.metrics.find((x) => x.metric === metric);
  return b.data ? (m?.last?.slice(0, 10) ?? today()) : null;
}

export function StepsDetail() {
  const [range, setRange] = useState<Range>("D");
  const [offset, setOffset] = useState(0);
  const anchor = useAnchor(STEPS);
  return (
    <div className="page">
      <NavBar to="/" title="Steps" sub={anchor ? dayTitle(anchor) : ""} />
      <Segmented value={range} options={RANGES} onChange={(r) => { setRange(r); setOffset(0); }} />
      <Pager offset={offset} setOffset={setOffset} />
      <MetricChartCard metric={STEPS} title="Count" color="var(--steps)" range={range} anchor={anchor} offset={offset} />
      <MetricChartCard metric={DISTANCE} title="Distance" color="var(--distance)" range={range} anchor={anchor} offset={offset} />
    </div>
  );
}

export function Pager({ offset, setOffset }: { offset: number; setOffset: (n: number) => void }) {
  return (
    <div className="pager">
      <button className="link" onClick={() => setOffset(offset + 1)} aria-label="Earlier">‹ Earlier</button>
      <button className="link" disabled={offset === 0} onClick={() => setOffset(Math.max(0, offset - 1))} aria-label="Later">Later ›</button>
    </div>
  );
}

export function DistanceDetail() {
  const { units } = useHealth();
  const [range, setRange] = useState<Range>("D");
  const [offset, setOffset] = useState(0);
  const anchor = useAnchor(DISTANCE);
  const d = useRange(DISTANCE, range, anchor, offset);
  const [sel, setSel] = useState<number | null>(null);
  const b = sel !== null ? d?.buckets[sel] : null;
  const val = b ? b.value : d?.summary;
  const [v, u] = conv(val ?? 0, d?.unit ?? "mi", units);
  return (
    <div className="page">
      <NavBar to="/" title="Step Distance" />
      <Segmented value={range} options={RANGES} onChange={(r) => { setRange(r); setOffset(0); setSel(null); }} />
      <section className="card">
        <p className="eyebrow">{b ? b.title.toUpperCase() : range === "D" ? "TOTAL" : "DAILY AVERAGE"}</p>
        <p className="big xl num" style={{ color: "var(--distance)" }}>{val == null ? "--" : v.toFixed(2)}<small>{u}</small></p>
        <p className="muted small">{d ? rangeLabel(range, d) : "…"}</p>
        <RangeChart buckets={d?.buckets ?? []} color="var(--distance)" height={240} gradient selected={sel} onSelect={setSel} onSwipe={(dir) => setOffset(Math.max(0, offset + dir))} />
        <Pager offset={offset} setOffset={setOffset} />
      </section>
      <div className="center"><Link to="/browse/activity" className="pill green">View All Steps Metrics</Link></div>
    </div>
  );
}

/* ---------- Activity day view ---------- */

function HourChart({ values, color, kind }: { values: number[]; color: string; kind: "bar" | "block" }) {
  const W = 340, H = 110, max = Math.max(...values, 1e-9), bw = W / 24;
  const peak = values.indexOf(Math.max(...values));
  return (
    <svg viewBox={`0 0 ${W} ${H + 16}`} className="hourchart" aria-hidden>
      <line x1="0" x2={W} y1={H} y2={H} stroke={color} strokeDasharray="1 4" strokeLinecap="round" strokeWidth="2" opacity=".6" />
      {kind === "bar" && max > 1e-6 && <text x={Math.min(peak * bw, W - 30)} y="10" className="axis" style={{ fill: color }}>{n0(max)}</text>}
      {values.map((v, i) =>
        kind === "block"
          ? <rect key={i} x={i * bw + 1.5} width={bw - 3} y={H - 26} height="22" rx="3" fill={color} opacity={v > 0 ? 1 : 0.18} />
          : v > 0 && <rect key={i} x={i * bw + bw * 0.22} width={bw * 0.56} y={H - (v / max) * (H - 16)} height={(v / max) * (H - 16)} rx="2" fill={color} />,
      )}
      {[0, 6, 12, 18].map((h) => <text key={h} x={h * bw} y={H + 14} className="axis">{String(h).padStart(2, "0")}:00</text>)}
    </svg>
  );
}

const mondayOf = (d: string) => addDays(d, -((new Date(d + "T12:00:00").getDay() + 6) % 7));

export function ActivityDay({ date }: { date: string }) {
  const [cal, setCal] = useState(false);
  const act = useApi<Activity>(`/dashboard/activity?end=${date}`);
  const mon = mondayOf(date);
  const week = useApi<{ days: RingDay[] }>(`/rings?start=${mon}&end=${addDays(mon, 6)}`);
  const mv = useApi<{ values: number[] }>(hourly(ENERGY, date));
  const ex = useApi<{ values: number[] }>(hourly(EXERCISE, date));
  const st = useApi<{ values: number[] }>(hourly(STAND, date));
  const r = act.data?.rings?.date === date ? act.data.rings : null;
  const title = `${date === today() ? "Today, " : ""}${new Date(date + "T12:00:00").toLocaleDateString([], { weekday: date === today() ? undefined : "short", month: "short", day: "numeric", year: "numeric" })}`;
  const hmTot = (m: number) => `${Math.floor(m / 60)}H ${Math.round(m % 60)}M`;
  return (
    <div className="page activityday">
      <NavBar to="/" title={title} right={<RoundBtn label="Calendar" onClick={() => setCal(true)}><CalIcon /></RoundBtn>} />
      <div className="weekstrip">
        {Array.from({ length: 7 }, (_, i) => {
          const d = addDays(mon, i);
          const rd = week.data?.days.find((x) => x.date === d);
          const future = d > today();
          return (
            <Link key={d} to={`/activity/${d}`} className={d === date ? "wd on" : "wd"}>
              <span className={d === today() ? "wl today" : "wl"}>{"MTWTFSS"[i]}</span>
              <FitRings v={rd && !future ? ringsOf(rd) : null} size={34} dim={future} />
            </Link>
          );
        })}
      </div>
      <div className="center bigrings"><FitRings v={r ? ringsOf(r) : null} size={250} /></div>
      <div className="act-charts">
      <section className="actsec">
        <div className="as-head"><div><p className="as-label">Move</p><p className="as-val num" style={{ color: "var(--move)" }}>{r ? `${n0(r.move_kcal)}/${n0(r.move_goal)}` : "--"}<small>CAL</small></p></div></div>
        <HourChart values={mv.data?.values ?? Array(24).fill(0)} color="var(--move)" kind="bar" />
        <p className="as-total">TOTAL {r ? n0(r.move_kcal) : 0} CAL</p>
      </section>
      <section className="actsec">
        <div className="as-head"><div><p className="as-label">Exercise</p><p className="as-val num" style={{ color: "var(--exercise)" }}>{r ? `${n0(r.exercise_min)}/${n0(r.exercise_goal)}` : "--"}<small>MIN</small></p></div></div>
        <HourChart values={ex.data?.values ?? Array(24).fill(0)} color="var(--exercise)" kind="bar" />
        <p className="as-total">TOTAL {hmTot(r?.exercise_min ?? 0)}</p>
      </section>
      <section className="actsec">
        <div className="as-head"><div><p className="as-label">Stand</p><p className="as-val num" style={{ color: "var(--stand)" }}>{r ? `${n0(r.stand_hours)}/${n0(r.stand_goal)}` : "--"}<small>HRS</small></p></div></div>
        <HourChart values={st.data?.values ?? Array(24).fill(0)} color="var(--stand)" kind="block" />
      </section>
      </div>
      {r && (
        <div className="tiles">
          <div className="tile"><b>Steps</b><p className="big num">{n0(r.steps)}</p></div>
          <div className="tile"><b>Workout</b><p className="big num">{week.data?.days.find((x) => x.date === date)?.workout ? "Yes" : "—"}</p></div>
        </div>
      )}
      {cal && <ActivityCalendar date={date} onClose={() => setCal(false)} />}
    </div>
  );
}

/* ---------- Activity calendar ---------- */

export function ActivityCalendar({ date, onClose }: { date: string; onClose: () => void }) {
  const end = useMemo(() => {
    const t = new Date(today() + "T12:00:00");
    return new Date(t.getFullYear(), t.getMonth() + 1, 0, 12).toISOString().slice(0, 10);
  }, []);
  const start = useMemo(() => {
    const t = new Date(today() + "T12:00:00");
    return new Date(t.getFullYear(), t.getMonth() - 11, 1, 12).toISOString().slice(0, 10);
  }, []);
  const data = useApi<{ days: RingDay[] }>(`/rings?start=${start}&end=${end}`);
  const by = useMemo(() => new Map((data.data?.days ?? []).map((d) => [d.date, d])), [data.data]);
  const scroller = useRef<HTMLDivElement>(null);
  const [title, setTitle] = useState(new Date(date + "T12:00:00").toLocaleDateString([], { month: "long", year: "numeric" }));
  const months = useMemo(() => {
    const out: { key: string; days: string[]; lead: number }[] = [];
    let d = start;
    while (d <= end) {
      const key = d.slice(0, 7);
      const days: string[] = [];
      while (d <= end && d.slice(0, 7) === key) { days.push(d); d = addDays(d, 1); }
      out.push({ key, days, lead: (new Date(days[0] + "T12:00:00").getDay() + 6) % 7 });
    }
    return out;
  }, [start, end]);
  useEffect(() => {
    const el = scroller.current?.querySelector(`[data-month="${date.slice(0, 7)}"]`) as HTMLElement | null;
    if (el && scroller.current) scroller.current.scrollTop = el.offsetTop - 8;
    onScroll();
  }, [date, data.data]); // eslint-disable-line react-hooks/exhaustive-deps
  const onScroll = () => {
    const s = scroller.current;
    if (!s) return;
    const ms = [...s.querySelectorAll<HTMLElement>("[data-month]")];
    const mid = s.scrollTop + s.clientHeight / 2;
    const cur = ms.filter((m) => m.offsetTop <= mid).pop();
    if (cur) setTitle(new Date(cur.dataset.month + "-01T12:00:00").toLocaleDateString([], { month: "long", year: "numeric" }));
  };
  return (
    <div className="fullsheet calendar" role="dialog" aria-label="Activity calendar">
      <div className="sheet-head">
        <RoundBtn label="Close" onClick={onClose}><XIcon /></RoundBtn>
        <b>{title}</b>
        <span style={{ width: 36 }} />
      </div>
      <div className="cal-dow">{[0, 1].map((k) => <div key={k} className={k ? "dowset dup" : "dowset"} aria-hidden={k ? true : undefined}>{"MTWTFSS".split("").map((c, i) => <span key={i}>{c}</span>)}</div>)}</div>
      <div className="cal-scroll" ref={scroller} onScroll={onScroll}>
        {months.map((m) => (
          <div key={m.key} data-month={m.key} className="cal-month">
            <div className="cal-grid">
              {Array.from({ length: m.lead }, (_, i) => <span key={"b" + i} />)}
              {m.days.map((d, i) => {
                const rd = by.get(d);
                const future = d > today();
                return (
                  <button key={d} className="cal-day" onClick={() => { onClose(); navigate(`/activity/${d}`); }} aria-label={d}>
                    {i === 0 && <span className="cal-mname">{new Date(d + "T12:00:00").toLocaleDateString([], { month: "short" })}</span>}
                    <span className={d === today() ? "cal-num today" : "cal-num"}>{Number(d.slice(8))}</span>
                    <FitRings v={rd && !future ? ringsOf(rd) : null} size={38} dim={future || !rd} />
                    <i className={rd?.workout ? "wdot on" : "wdot"} />
                  </button>
                );
              })}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}


export function WorkoutDetail({ id }: { id: string }) {
  const w = useApi<import("../api").Workout>(`/workouts/${id}`);
  if (w.error) return <div className="page"><p className="muted">Workout not found.</p><Link to="/">Back</Link></div>;
  if (!w.data) return <div className="page"><p className="muted">Loading…</p></div>;
  const x = w.data;
  const km = x.distance_m != null ? x.distance_m / 1000 : null;
  return (
    <div className="page">
      <LargeTitle title={prettyWorkout(x.activity_type)} eyebrow={(x.local_start ?? x.start_utc).slice(0, 16)} />
      <div className="card">
        <dl className="kv">
          <div><dt>Duration</dt><dd className="num">{x.duration_min != null ? `${Math.round(x.duration_min)} min` : "—"}</dd></div>
          <div><dt>Active energy</dt><dd className="num">{x.active_kcal != null ? `${Math.round(x.active_kcal)} kcal` : "—"}</dd></div>
          <div><dt>Total energy</dt><dd className="num">{x.total_kcal != null ? `${Math.round(x.total_kcal)} kcal` : "—"}</dd></div>
          <div><dt>Distance</dt><dd className="num">{km != null ? `${km.toFixed(2)} km` : "—"}</dd></div>
          <div><dt>Avg HR</dt><dd className="num">{x.avg_hr != null ? `${Math.round(x.avg_hr)} bpm` : "—"}</dd></div>
          <div><dt>Max HR</dt><dd className="num">{x.max_hr != null ? `${Math.round(x.max_hr)} bpm` : "—"}</dd></div>
          <div><dt>Source</dt><dd>{x.source || "—"} <span className="muted">({x.channel})</span></dd></div>
        </dl>
      </div>
      <Link to="/" className="card linkrow">Back to Summary</Link>
    </div>
  );
}
