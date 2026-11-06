// Summary (Fitness-style), Activity day view + calendar, Steps & Distance details.
import { useEffect, useMemo, useRef, useState } from "react";
import { Activity, api } from "../api";
import { Link, navigate } from "../router";
import { useHealth, conv } from "./ctx";
import { DISTANCE, ENERGY, STEPS, useApi, useBrowse } from "./meta";
import { Board } from "./Board";
import { addDays, rangeLabel, Range, RangeChart, Segmented, today, useRange, dayTitle } from "./range";
import { isFutureDay, mondayOf, replayGate, shiftDate } from "./day-nav";
import { FitRings, GoalFrom, noGoals, ringsOf } from "./rings";
import { NoGoalsAlert } from "./Goals";
import { useBarMotion } from "./use-bar-motion";
import { useDaySlide } from "./use-day-slide";
import { usePrefersReducedMotion } from "./use-ring-motion";
import { CalIcon, LargeTitle, NavBar, RoundBtn, XIcon } from "./ui";

export const EXERCISE = "HKQuantityTypeIdentifierAppleExerciseTime";
export const STAND = "HKCategoryTypeIdentifierAppleStandHour";
export type RingDay = { date: string; move_kcal: number; move_goal: number | null; move_goal_from: string | null; move_goal_source?: string | null; exercise_min: number; exercise_goal: number | null; exercise_goal_from: string | null; exercise_goal_source?: string | null; stand_hours: number; stand_goal: number | null; stand_goal_from: string | null; stand_goal_source?: string | null; workout: boolean };

const n0 = (v: number) => Math.round(v).toLocaleString();
const hourly = (m: string, d?: string | null) => `/hourly?metric=${encodeURIComponent(m)}${d ? `&date=${d}` : ""}`;
const prettyWorkout = (m: string) => m.replace(/^HKWorkoutActivityType/, "").replace(/([a-z])([A-Z])/g, "$1 $2");


export default function Summary() {
  const act = useApi<Activity>("/dashboard/activity");
  const r = act.data?.rings;
  const showAlert = noGoals(r) && (r == null || r.date === today());
  return (
    <>
      <NoGoalsAlert show={showAlert} />
      <Board which="summary" />
    </>
  );
}

export function WorkoutIcon() {
  return <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden><circle cx="14" cy="4" r="2.4" /><path d="M10.5 7.5 7 10l1 1.6 2.6-1.6 1 3.5-3.6 4.2 1.5 1.3 4-4.4 1.5 2.4v4h2v-4.6l-2-3.4.8-3.2 1.4 1.8H20v-2h-2.8l-2.4-3a2 2 0 0 0-1.6-.8Z" /></svg>;
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

const zeros = () => Array(24).fill(0) as number[];

type DayPack = {
  rings: Activity["rings"];
  days: RingDay[];
  move: number[];
  exercise: number[];
  stand: number[];
};

function useActivityPacks(dates: string[]) {
  const [packs, setPacks] = useState<Record<string, DayPack>>({});
  const key = dates.join("|");
  useEffect(() => {
    let dead = false;
    for (const date of dates) {
      if (packs[date]) continue;
      const mon = mondayOf(date);
      Promise.all([
        api<Activity>(`/dashboard/activity?end=${date}`),
        api<{ days: RingDay[] }>(`/rings?start=${mon}&end=${shiftDate(mon, 6)}`),
        api<{ values: number[] }>(hourly(ENERGY, date)),
        api<{ values: number[] }>(hourly(EXERCISE, date)),
        api<{ values: number[] }>(hourly(STAND, date)),
      ]).then(([act, week, mv, ex, st]) => {
        if (dead) return;
        const pack: DayPack = {
          rings: act.rings?.date === date ? act.rings : null,
          days: week.days ?? [],
          move: mv.values?.length === 24 ? mv.values : zeros(),
          exercise: ex.values?.length === 24 ? ex.values : zeros(),
          stand: st.values?.length === 24 ? st.values : zeros(),
        };
        setPacks((s) => (s[date] ? s : { ...s, [date]: pack }));
      }, () => {});
    }
    return () => { dead = true; };
    // Missing packs are filled once per date. A newer date list drops late writes via `dead`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  return packs;
}

function HourChart({ values, color, kind, replayKey, play, settle }: { values: number[]; color: string; kind: "bar" | "block"; replayKey: string; play: boolean; settle: boolean }) {
  const W = 340, H = 110, bw = W / 24;
  const motion = useBarMotion(values, replayKey, play, settle);
  const shown = motion.shown;
  const draw = kind === "bar" ? shown : values;
  const max = Math.max(...values, 1e-9);
  const peak = values.indexOf(Math.max(...values));
  return (
    <svg ref={motion.ref} viewBox={`0 0 ${W} ${H + 16}`} className="hourchart" aria-hidden>
      <line x1="0" x2={W} y1={H} y2={H} stroke={color} strokeDasharray="1 4" strokeLinecap="round" strokeWidth="2" opacity=".6" />
      {kind === "bar" && max > 1e-6 && (draw[peak] ?? 0) > max * 0.08 && <text x={Math.min(peak * bw, W - 30)} y={Math.max(12, H - ((draw[peak] ?? 0) / max) * (H - 16) - 4)} className="axis" style={{ fill: color }}>{n0(max)}</text>}
      {draw.map((v, i) =>
        kind === "block"
          ? <rect key={i} x={i * bw + 1.5} width={bw - 3} y={H - 26} height="22" rx="3" fill={color} opacity={v > 0 ? 1 : 0.18} />
          : v > 0 && <rect key={i} x={i * bw + bw * 0.22} width={bw * 0.56} y={H - (v / max) * (H - 16)} height={(v / max) * (H - 16)} rx="2" fill={color} />,
      )}
      {[0, 6, 12, 18].map((h) => <text key={h} x={h * bw} y={H + 14} className="axis">{String(h).padStart(2, "0")}:00</text>)}
    </svg>
  );
}

function DayChev({ dir }: { dir: -1 | 1 }) {
  return (
    <svg width="22" height="22" viewBox="0 0 24 24" aria-hidden focusable="false">
      <path d={dir < 0 ? "M14.5 6.5 9 12l5.5 5.5" : "M9.5 6.5 15 12l-5.5 5.5"} fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function goDay(date: string) {
  navigate(`/activity/${date}`, { scroll: false });
}

function WeekStrip({ date, pack }: { date: string; pack?: DayPack }) {
  const mon = mondayOf(date);
  return (
    <div className="weekstrip" role="group" aria-label="Days this week">
      {Array.from({ length: 7 }, (_, i) => {
        const d = shiftDate(mon, i);
        const rd = pack?.days.find((x) => x.date === d);
        const later = isFutureDay(d, today());
        return (
          <a key={d} href={`/activity/${d}`} className={d === date ? "wd on" : "wd"} aria-current={d === date ? "date" : undefined} onClick={(e) => { if (e.metaKey || e.ctrlKey || e.shiftKey) return; e.preventDefault(); goDay(d); }}>
            <span className={d === today() ? "wl today" : "wl"}>{"MTWTFSS"[i]}</span>
            <FitRings v={rd && !later ? ringsOf(rd) : null} size={34} dim={later} replayKey={d} settle />
          </a>
        );
      })}
    </div>
  );
}

function DayFace({ date, pack, play, settle }: { date: string; pack?: DayPack; play: boolean; settle: boolean }) {
  const future = isFutureDay(date, today());
  const r = pack?.rings ?? null;
  const showRings = !future && r ? ringsOf(r) : null;
  return (
    <>
      <div className="center bigrings"><FitRings v={play || settle ? showRings : null} size={250} replayKey={date} settle={settle || !play} /></div>
      {r && <GoalFrom r={r} className="center" />}
      {(r || date === today()) && <p className="muted small center"><Link to="/account/goals">{noGoals(r) ? "Set goals" : "Edit goals"}</Link></p>}
      <div className="act-charts">
        <section className="actsec">
          <div className="as-head">
            <p className="as-label" style={{ color: "var(--move)" }}>Move</p>
            <p className="as-of muted">{r ? `${n0(r.move_kcal)} of ${r.move_goal != null ? n0(r.move_goal) : "—"} cal` : "--"}</p>
          </div>
          <HourChart values={pack?.move ?? zeros()} color="var(--move)" kind="bar" replayKey={date} play={play} settle={settle} />
        </section>
        <section className="actsec">
          <div className="as-head">
            <p className="as-label" style={{ color: "var(--exercise)" }}>Exercise</p>
            <p className="as-of muted">{r ? `${n0(r.exercise_min)} of ${r.exercise_goal != null ? n0(r.exercise_goal) : "—"} min` : "--"}</p>
          </div>
          <HourChart values={pack?.exercise ?? zeros()} color="var(--exercise)" kind="bar" replayKey={date} play={play} settle={settle} />
        </section>
        <section className="actsec">
          <div className="as-head">
            <p className="as-label" style={{ color: "var(--stand)" }}>Stand</p>
            <p className="as-of muted">{r ? `${n0(r.stand_hours)} of ${r.stand_goal != null ? n0(r.stand_goal) : "—"} hr` : "--"}</p>
          </div>
          <HourChart values={pack?.stand ?? zeros()} color="var(--stand)" kind="block" replayKey={date} play={false} settle />
        </section>
      </div>
      {r && (
        <div className="tiles">
          <div className="tile"><b>Steps</b><p className="big num">{n0(r.steps)}</p></div>
          <div className="tile"><b>Workout</b><p className="big num">{pack?.days.find((x) => x.date === date)?.workout ? "Yes" : "—"}</p></div>
        </div>
      )}
    </>
  );
}

export function ActivityDay({ date }: { date: string }) {
  const [cal, setCal] = useState(false);
  const reduced = usePrefersReducedMotion();
  const slide = useDaySlide(date, reduced);
  const packs = useActivityPacks(slide.phase === "run" ? [slide.from, slide.to] : [slide.to]);
  const [go, setGo] = useState(false);
  useEffect(() => {
    if (slide.phase !== "run") { setGo(false); return; }
    setGo(false);
    let raf2 = 0;
    const raf1 = requestAnimationFrame(() => { raf2 = requestAnimationFrame(() => setGo(true)); });
    return () => { cancelAnimationFrame(raf1); cancelAnimationFrame(raf2); };
  }, [slide.phase, slide.from, slide.to]);
  const titleDate = slide.phase === "run" ? slide.to : date;
  const title = `${titleDate === today() ? "Today, " : ""}${new Date(titleDate + "T12:00:00").toLocaleDateString([], { weekday: titleDate === today() ? undefined : "short", month: "short", day: "numeric", year: "numeric" })}`;
  const headRings = packs[titleDate]?.rings ?? null;
  const showAlert = titleDate === today() && noGoals(headRings);
  const order = slide.dir === -1 ? [slide.to, slide.from] : [slide.from, slide.to];
  const shift = slide.dir === -1 ? (go ? "translateX(0)" : "translateX(-50%)") : (go ? "translateX(-50%)" : "translateX(0)");
  const sliding = slide.phase === "run" && slide.dir !== 0;
  const gate = replayGate(sliding ? "run" : "idle", reduced);
  const track = (part: "strip" | "body") => sliding ? (
    <div className={go && !reduced ? "day-track on" : "day-track"} style={{ transform: shift }}>
      {order.map((d) => (
        <div className="day-pane" key={`${part}:${slide.from}->${slide.to}:${d}`} aria-hidden={d !== slide.to}>
          {part === "strip"
            ? <WeekStrip date={d} pack={packs[d]} />
            : <DayFace date={d} pack={packs[d]} play={false} settle={d === slide.from} />}
        </div>
      ))}
    </div>
  ) : part === "strip"
    ? <WeekStrip date={date} pack={packs[date]} />
    : <DayFace date={date} pack={packs[date]} play={gate === "play"} settle={gate !== "play"} />;
  return (
    <div className="page activityday">
      <NavBar to="/" title={title} right={<RoundBtn label="Calendar" onClick={() => setCal(true)}><CalIcon /></RoundBtn>} />
      <NoGoalsAlert show={showAlert} />
      <div className="daynav">
        <button type="button" className="daybtn" aria-label="Previous day" onClick={() => goDay(shiftDate(date, -1))}><DayChev dir={-1} /></button>
        <div className="day-clip" aria-live="polite">{track("strip")}</div>
        <button type="button" className="daybtn" aria-label="Next day" onClick={() => goDay(shiftDate(date, 1))}><DayChev dir={1} /></button>
      </div>
      <div className="day-clip">{track("body")}</div>
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
                    <FitRings v={rd && !future ? ringsOf(rd) : null} size={38} dim={future || !rd} replayKey={d} />
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
