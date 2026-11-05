import { useEffect, useState } from "react";
import { Activity, api, Body, Dashboard as Dash, explain, Heart, Metric, SeriesData, Sleep, Widget } from "../api";
import { Bars, Hypnogram, Lines, Rings, STAGES } from "../components/charts";
import { Link } from "../router";

const n0 = (v: number) => Math.round(v).toLocaleString();
const hm = (min: number) => `${Math.floor(min / 60)}h ${String(Math.round(min % 60)).padStart(2, "0")}m`;
const avg = (xs: number[]) => (xs.length ? xs.reduce((a, b) => a + b, 0) / xs.length : 0);
const longDate = (d: string) =>
  new Date(d + "T12:00:00").toLocaleDateString([], { weekday: "long", month: "long", day: "numeric" });
const fmtVal = (v: number) => (Math.abs(v) >= 100 ? n0(v) : v.toFixed(Math.abs(v) >= 10 ? 1 : 2));

/** Fetches a path; refetches when it changes. */
function useApi<T>(path: string | null) {
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

const SPECIAL: Record<string, string> = { activity: "Activity rings", sleep: "Sleep stages", body: "Body composition", heart: "Heart" };
const KIND_LABEL: Record<string, string> = { line: "Line chart", bar: "Bar chart", stat: "Single stat", ...SPECIAL };
const RANGES = [7, 14, 30, 90, 180, 365, 730];
const newId = () => Math.random().toString(36).slice(2, 10);

export default function Dashboard() {
  const [boards, setBoards] = useState<Dash[] | null>(null);
  const [current, setCurrent] = useState<number | null>(null);
  const [editing, setEditing] = useState(false);
  const [adding, setAdding] = useState<Widget | null>(null);
  const [metrics, setMetrics] = useState<Metric[]>([]);
  const [err, setErr] = useState("");
  const [failed, setFailed] = useState(false);
  const activity = useApi<Activity>("/dashboard/activity");

  useEffect(() => {
    api<{ dashboards: Dash[] }>("/dashboards")
      .then((r) => {
        setBoards(r.dashboards);
        setCurrent(r.dashboards[0]?.id ?? null);
      })
      .catch(() => setFailed(true));
    api<{ metrics: Metric[] }>("/metrics").then((r) => setMetrics(r.metrics), () => {});
  }, []);

  if (failed) return <div className="card empty"><p>Couldn't load your dashboard. Try reloading.</p></div>;
  if (!boards) return <p className="muted">Loading…</p>;
  const board = boards.find((b) => b.id === current) ?? boards[0];

  const replace = (d: Dash) => setBoards((bs) => bs!.map((b) => (b.id === d.id ? d : b)));
  const save = async (patch: { name?: string; widgets?: Widget[] }) => {
    if (!board) return;
    setErr("");
    // Optimistic: show the change now, roll back if the server says no.
    const before = board;
    replace({ ...board, ...patch });
    try {
      replace((await api<{ dashboard: Dash }>(`/dashboards/${board.id}`, { method: "PUT", body: patch })).dashboard);
    } catch (e) {
      replace(before);
      setErr(explain(e));
    }
  };
  const create = async () => {
    const name = prompt("Name for the new dashboard", "My dashboard")?.trim();
    if (!name) return;
    try {
      const d = (await api<{ dashboard: Dash }>("/dashboards", { method: "POST", body: { name, widgets: [] } })).dashboard;
      setBoards([...boards, d]);
      setCurrent(d.id);
      setEditing(true);
    } catch (e) {
      setErr(explain(e));
    }
  };
  const rename = () => {
    const name = prompt("Rename dashboard", board.name)?.trim();
    if (name && name !== board.name) save({ name });
  };
  const remove = async () => {
    if (!confirm(`Delete the "${board.name}" dashboard? Your health data is not affected.`)) return;
    try {
      await api(`/dashboards/${board.id}`, { method: "DELETE" });
      const rest = boards.filter((b) => b.id !== board.id);
      if (rest.length) {
        setBoards(rest);
        setCurrent(rest[0].id);
      } else {
        // Deleting the last one brings back the default Summary.
        const r = await api<{ dashboards: Dash[] }>("/dashboards");
        setBoards(r.dashboards);
        setCurrent(r.dashboards[0].id);
      }
    } catch (e) {
      setErr(explain(e));
    }
  };
  const ws = board?.widgets ?? [];
  const move = (i: number, by: number) => {
    const j = i + by;
    if (j < 0 || j >= ws.length) return;
    const next = [...ws];
    [next[i], next[j]] = [next[j], next[i]];
    save({ widgets: next });
  };
  const resize = (i: number, by: number) =>
    save({ widgets: ws.map((w, k) => (k === i ? { ...w, size: Math.min(4, Math.max(1, w.size + by)) } : w)) });
  const drop = (i: number) => save({ widgets: ws.filter((_, k) => k !== i) });
  const upsert = (w: Widget) => {
    const exists = ws.some((x) => x.id === w.id);
    save({ widgets: exists ? ws.map((x) => (x.id === w.id ? w : x)) : [...ws, w] });
    setAdding(null);
  };

  const noData = metrics.length === 0;
  return (
    <div className="dash">
      <header className="pagehead dashhead">
        <div>
          <p className="eyebrow">{activity.data?.date ? longDate(activity.data.date) : "Dashboards"}</p>
          <h1>{board?.name ?? "Summary"}</h1>
        </div>
        <div className="row wrap dashtools">
          {boards.length > 1 && (
            <div className="segmented" role="tablist">
              {boards.map((b) => (
                <button key={b.id} className={b.id === board.id ? "on" : ""} onClick={() => setCurrent(b.id)}>
                  {b.name}
                </button>
              ))}
            </div>
          )}
          <button className={editing ? "primary small" : "small"} onClick={() => { setEditing(!editing); setAdding(null); }}>
            {editing ? "Done" : "Edit"}
          </button>
        </div>
      </header>
      {editing && (
        <div className="card editbar row wrap">
          <button className="small primary" onClick={() => setAdding({ id: newId(), kind: "line", days: 30, bucket: "day", size: 2 })}>
            + Add widget
          </button>
          <button className="small" onClick={rename}>Rename</button>
          <button className="small" onClick={create}>+ New dashboard</button>
          <button className="small ghost danger" onClick={remove}>Delete dashboard</button>
          {err && <span className="error small">{err}</span>}
        </div>
      )}
      {adding && <WidgetForm w={adding} metrics={metrics} onSave={upsert} onCancel={() => setAdding(null)} />}
      {noData && !editing ? (
        <div className="card empty hero">
          <h2>No health data yet</h2>
          <p className="muted">
            Set up Health Auto Export to send data automatically, or import the <code>export.zip</code> from the Health app.
          </p>
          <div className="row">
            <Link to="/help/health-auto-export" className="button primary">Health Auto Export setup</Link>
            <Link to="/settings/import" className="button">Import an export</Link>
          </div>
        </div>
      ) : ws.length === 0 ? (
        <div className="card empty">
          <p className="muted">This dashboard is empty.</p>
          {!editing && <button onClick={() => setEditing(true)}>Add widgets</button>}
        </div>
      ) : (
        <div className="dgrid">
          {ws.map((w, i) => (
            <div key={w.id} className={`widget${editing ? " editing" : ""}`} data-size={w.size} style={{ "--span": w.size } as React.CSSProperties}>
              {editing && (
                <div className="wtools">
                  <button className="small ghost" title="Move earlier" onClick={() => move(i, -1)} disabled={i === 0}>←</button>
                  <button className="small ghost" title="Move later" onClick={() => move(i, 1)} disabled={i === ws.length - 1}>→</button>
                  <button className="small ghost" title="Narrower" onClick={() => resize(i, -1)} disabled={w.size <= 1}>−</button>
                  <span className="muted small">{w.size}/4</span>
                  <button className="small ghost" title="Wider" onClick={() => resize(i, 1)} disabled={w.size >= 4}>+</button>
                  <button className="small ghost" title="Edit" onClick={() => setAdding(w)}>✎</button>
                  <button className="small ghost danger" title="Remove" onClick={() => drop(i)}>✕</button>
                </div>
              )}
              <WidgetView w={w} metrics={metrics} />
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function WidgetForm(props: { w: Widget; metrics: Metric[]; onSave: (w: Widget) => void; onCancel: () => void }) {
  const [w, setW] = useState<Widget>(props.w);
  const chart = ["line", "bar", "stat"].includes(w.kind);
  const set = (p: Partial<Widget>) => setW({ ...w, ...p });
  const m = props.metrics.find((x) => x.metric === w.metric);
  const valid = !chart || !!w.metric;
  return (
    <div className="card wform">
      <h2>{props.w.metric || SPECIAL[props.w.kind] ? "Edit widget" : "Add widget"}</h2>
      <div className="row wrap">
        <label>
          Type
          <select value={w.kind} onChange={(e) => set({ kind: e.target.value as Widget["kind"] })}>
            {Object.entries(KIND_LABEL).map(([k, l]) => <option key={k} value={k}>{l}</option>)}
          </select>
        </label>
        {chart && (
          <>
            <label className="grow">
              Metric
              <select value={w.metric ?? ""} onChange={(e) => set({ metric: e.target.value || undefined, agg: undefined })}>
                <option value="">Choose a metric…</option>
                {props.metrics.map((x) => (
                  <option key={x.metric} value={x.metric}>
                    {x.name}{x.unit ? ` (${x.unit})` : ""} · {x.count.toLocaleString()}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Range
              <select value={w.days} onChange={(e) => set({ days: Number(e.target.value) })}>
                {RANGES.map((d) => <option key={d} value={d}>{d < 365 ? `${d} days` : `${d / 365} year${d > 365 ? "s" : ""}`}</option>)}
              </select>
            </label>
            {w.kind !== "stat" && (
              <label>
                Group by
                <select value={w.bucket} onChange={(e) => set({ bucket: e.target.value as Widget["bucket"] })}>
                  <option value="day">Day</option>
                  <option value="week">Week</option>
                  <option value="month">Month</option>
                </select>
              </label>
            )}
            <label>
              Combine
              <select value={w.agg ?? ""} onChange={(e) => set({ agg: (e.target.value || undefined) as Widget["agg"] })}>
                <option value="">Auto{m ? ` (${m.agg})` : ""}</option>
                <option value="sum">Total</option>
                <option value="avg">Average</option>
                <option value="count">Count of samples</option>
              </select>
            </label>
          </>
        )}
        <label>
          Title
          <input value={w.title ?? ""} placeholder={chart ? m?.name ?? "" : SPECIAL[w.kind]} maxLength={80} onChange={(e) => set({ title: e.target.value || undefined })} />
        </label>
      </div>
      <div className="row">
        <button className="primary" disabled={!valid} onClick={() => props.onSave({ ...w, metric: chart ? w.metric : undefined })}>Save widget</button>
        <button className="ghost" onClick={props.onCancel}>Cancel</button>
      </div>
    </div>
  );
}

function WidgetView({ w, metrics }: { w: Widget; metrics: Metric[] }) {
  switch (w.kind) {
    case "activity":
      return <Loaded path="/dashboard/activity" render={(a: Activity) => <ActivityCard a={a} />} />;
    case "heart":
      return <Loaded path={`/dashboard/heart?days=${w.days}`} render={(h: Heart) => <HeartCard h={h} />} />;
    case "sleep":
      return <Loaded path="/dashboard/sleep?days=14" render={(s: Sleep) => <SleepCard s={s} />} />;
    case "body":
      return <Loaded path={`/dashboard/body?days=${Math.max(w.days, 90)}`} render={(b: Body) => <BodyCard b={b} />} />;
    default:
      return <MetricCard w={w} metrics={metrics} />;
  }
}

function Loaded<T>(props: { path: string; render: (d: T) => React.ReactNode }) {
  const { data, error } = useApi<T>(props.path);
  if (error) return <section className="card"><p className="muted">Couldn't load this widget.</p></section>;
  if (!data) return <section className="card"><p className="muted">Loading…</p></section>;
  return <>{props.render(data)}</>;
}

function MetricCard({ w, metrics }: { w: Widget; metrics: Metric[] }) {
  // A stat compares the latest day against the rest of the range.
  const bucket = w.kind === "stat" ? "day" : w.bucket;
  const q = new URLSearchParams({ metric: w.metric ?? "", days: String(w.days), bucket });
  if (w.agg) q.set("agg", w.agg);
  const { data, error } = useApi<SeriesData>(`/series?${q}`);
  const known = metrics.find((m) => m.metric === w.metric);
  const title = w.title || data?.name || known?.name || "Metric";
  const rawUnit = data?.unit ?? "";
  const unit = ({ count: "", "count/min": "bpm" } as Record<string, string>)[rawUnit] ?? rawUnit;
  // Apple stores percentages as fractions (0.97); show 97.
  const pct = rawUnit === "%" && (data?.points ?? []).every((p) => p.value <= 1);
  const pts = (data?.points ?? []).map((p) => (pct ? { ...p, value: p.value * 100 } : p));
  const latest = pts[pts.length - 1];
  const prior = avg(pts.slice(0, -1).map((p) => p.value));
  const per = bucket;
  const perly = { day: "daily", week: "weekly", month: "monthly" }[bucket];
  const aggWord = data?.agg === "avg" ? "average" : data?.agg === "count" ? "samples" : "total";
  return (
    <section className={`card metric ${w.kind}`}>
      <div className="cardhead">
        <h2>{title}</h2>
        {latest && w.kind !== "stat" && <span className="big">{fmtVal(latest.value)}<small> {unit}</small></span>}
      </div>
      {error ? (
        <p className="muted">Couldn't load this widget.</p>
      ) : !data ? (
        <p className="muted">Loading…</p>
      ) : pts.length === 0 ? (
        <p className="muted empty-inline">No data in this range.</p>
      ) : w.kind === "stat" ? (
        <div className="stat">
          <span className="statval">{fmtVal(latest.value)}<small> {unit}</small></span>
          {pts.length > 1 && prior !== 0 && (
            <span className={latest.value >= prior ? "trend up" : "trend down"}>
              {latest.value >= prior ? "▲" : "▼"} {Math.abs(((latest.value - prior) / prior) * 100).toFixed(0)}% vs {w.days}-day avg {fmtVal(prior)}
            </span>
          )}
          <p className="muted foot">{latest.date} · daily {aggWord}</p>
        </div>
      ) : (
        <>
          {w.kind === "bar" ? (
            <Bars data={pts} color="var(--accent)" unit={unit} />
          ) : (
            <Lines series={[{ data: pts, color: "var(--chart-2)", label: title }]} decimals={Math.abs(avg(pts.map((p) => p.value))) < 10 ? 1 : 0} />
          )}
          <p className="muted foot">
            {perly} {aggWord} · {pts.length}-{per} average {fmtVal(avg(pts.map((p) => p.value)))} {unit}
          </p>
        </>
      )}
    </section>
  );
}

function Empty({ what }: { what: string }) {
  return <p className="muted empty-inline">No {what} data in your imports yet.</p>;
}

function ActivityCard({ a }: { a: Activity }) {
  const r = a.rings;
  return (
    <section className="card activity">
      <h2>Activity</h2>
      {!r ? (
        <Empty what="activity" />
      ) : (
        <div className="rings-row">
          <Rings move={[r.move_kcal, r.move_goal]} exercise={[r.exercise_min, r.exercise_goal]} stand={[r.stand_hours, r.stand_goal]} size={200} />
          <dl className="ring-stats">
            <div>
              <dt>Move</dt>
              <dd style={{ color: "var(--move)" }}>{n0(r.move_kcal)}<small>/{n0(r.move_goal)} kcal</small></dd>
            </div>
            <div>
              <dt>Exercise</dt>
              <dd style={{ color: "var(--exercise)" }}>{n0(r.exercise_min)}<small>/{r.exercise_goal} min</small></dd>
            </div>
            <div>
              <dt>Stand</dt>
              <dd style={{ color: "var(--stand)" }}>{n0(r.stand_hours)}<small>/{r.stand_goal} hr</small></dd>
            </div>
          </dl>
        </div>
      )}
    </section>
  );
}

function HeartCard({ h }: { h: Heart }) {
  const latest = (xs: { value: number }[]) => (xs.length ? xs[xs.length - 1].value : null);
  const rhr = latest(h.resting);
  const hrv = latest(h.hrv);
  return (
    <section className="card">
      <div className="cardhead">
        <h2>Heart</h2>
        {rhr !== null && <span className="big" style={{ color: "var(--heart)" }}>{n0(rhr)}<small> bpm resting</small></span>}
      </div>
      {h.daily.length === 0 && h.resting.length === 0 ? (
        <Empty what="heart" />
      ) : (
        <>
          <Lines
            series={[{ data: h.resting, color: "var(--heart)", label: "Resting" }]}
            band={h.daily.map((d) => ({ date: d.date, lo: d.min, hi: d.max }))}
            bandColor="var(--heart)"
          />
          <p className="muted foot">
            Line: resting heart rate · band: daily range
            {hrv !== null && <> · HRV {n0(hrv)} ms</>}
          </p>
        </>
      )}
    </section>
  );
}

function SleepCard({ s }: { s: Sleep }) {
  const last = s.nights[s.nights.length - 1];
  const asleep = (n: Sleep["nights"][number]) => n.asleep_min;
  return (
    <section className="card sleep">
      <div className="cardhead">
        <h2>Sleep</h2>
        {last && <span className="big" style={{ color: "var(--core)" }}>{hm(asleep(last))}</span>}
      </div>
      {!last ? (
        <Empty what="sleep" />
      ) : (
        <>
          <Hypnogram segments={s.last_night} />
          <div className="legend">
            {STAGES.map((st) => {
              const key = { Awake: "awake_min", AsleepREM: "rem_min", AsleepCore: "core_min", AsleepDeep: "deep_min" }[st.key] as keyof typeof last;
              return (
                <span key={st.key}>
                  <i style={{ background: st.color }} />
                  {st.label} {hm(Number(last[key]))}
                </span>
              );
            })}
          </div>
          <p className="muted foot">Samples ending {last.date} from {last.source || "unknown source"}{last.in_bed_min > 0 ? ` · in bed ${hm(last.in_bed_min)}` : ""}</p>
        </>
      )}
    </section>
  );
}

function BodyCard({ b }: { b: Body }) {
  const w = b.weight.points;
  const fat = b.body_fat.points;
  const latest = w[w.length - 1];
  const delta = w.length > 1 ? latest.value - w[0].value : 0;
  // Apple stores body fat as a fraction (0.21); show percent.
  const pct = (v: number) => (b.body_fat.unit === "%" && v <= 1 ? v * 100 : v);
  return (
    <section className="card body">
      <div className="cardhead">
        <h2>Body</h2>
        {latest && (
          <span className="big">
            {latest.value.toFixed(1)}<small> {b.weight.unit ?? ""}</small>
            {fat.length > 0 && <small> · {pct(fat[fat.length - 1].value).toFixed(1)}% fat</small>}
          </span>
        )}
      </div>
      {w.length === 0 && fat.length === 0 ? (
        <Empty what="body measurement" />
      ) : (
        <>
          <Lines series={[{ data: w, color: "var(--chart-2)", label: "Weight" }]} decimals={1} height={120} />
          {w.length > 1 && (
            <p className="muted foot">
              {delta >= 0 ? "+" : ""}{delta.toFixed(1)} {b.weight.unit} over {w.length} weigh-ins
            </p>
          )}
        </>
      )}
    </section>
  );
}
