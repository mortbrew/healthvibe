import { useEffect, useState } from "react";
import { Activity, api, Body, Heart, Sleep } from "../api";
import { Bars, Hypnogram, Lines, Rings, STAGES } from "../components/charts";
import { Link } from "../router";

const n0 = (v: number) => Math.round(v).toLocaleString();
const hm = (min: number) => `${Math.floor(min / 60)}h ${String(Math.round(min % 60)).padStart(2, "0")}m`;
const avg = (xs: number[]) => (xs.length ? xs.reduce((a, b) => a + b, 0) / xs.length : 0);
const longDate = (d: string) =>
  new Date(d + "T12:00:00").toLocaleDateString([], { weekday: "long", month: "long", day: "numeric" });

type Data = { activity: Activity; heart: Heart; sleep: Sleep; body: Body };

export default function Dashboard() {
  const [data, setData] = useState<Data | null>(null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    Promise.all([
      api<Activity>("/dashboard/activity"),
      api<Heart>("/dashboard/heart?days=30"),
      api<Sleep>("/dashboard/sleep?days=14"),
      api<Body>("/dashboard/body?days=90"),
    ])
      .then(([activity, heart, sleep, body]) => setData({ activity, heart, sleep, body }))
      .catch(() => setFailed(true));
  }, []);

  if (failed) return <div className="card empty"><p>Couldn't load your dashboard. Try reloading.</p></div>;
  if (!data) return <p className="muted">Loading…</p>;
  const { activity, heart, sleep, body } = data;
  const nothing =
    !activity.rings && heart.daily.length === 0 && sleep.nights.length === 0 && body.weight.points.length === 0;

  return (
    <div className="dash">
      <header className="pagehead">
        <p className="eyebrow">{activity.date ? longDate(activity.date) : "Summary"}</p>
        <h1>Summary</h1>
      </header>
      {nothing ? (
        <div className="card empty hero">
          <h2>No health data yet</h2>
          <p className="muted">
            Import the <code>export.zip</code> from the Health app, or set up the iPhone Shortcut to send data
            automatically.
          </p>
          <div className="row">
            <Link to="/import" className="button primary">Import an export</Link>
            <Link to="/help/shortcut" className="button">Shortcut setup guide</Link>
          </div>
        </div>
      ) : (
        <div className="grid">
          <ActivityCard a={activity} />
          <StepsCard a={activity} />
          <HeartCard h={heart} />
          <SleepCard s={sleep} />
          <BodyCard b={body} />
        </div>
      )}
    </div>
  );
}

function Empty({ what }: { what: string }) {
  return <p className="muted empty-inline">No {what} data in your imports yet.</p>;
}

function ActivityCard({ a }: { a: Activity }) {
  const r = a.rings;
  return (
    <section className="card span2 activity">
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

function StepsCard({ a }: { a: Activity }) {
  const days = a.steps;
  return (
    <section className="card span2">
      <div className="cardhead">
        <h2>Steps</h2>
        {a.rings && <span className="big">{n0(a.rings.steps)}<small> today</small></span>}
      </div>
      {days.length === 0 ? (
        <Empty what="step" />
      ) : (
        <>
          <Bars data={days} color="var(--accent)" goal={10000} unit="steps" />
          <p className="muted foot">{days.length}-day average {n0(avg(days.map((d) => d.value)))} · dashed line: 10,000</p>
        </>
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
          <p className="muted foot">{s.nights.length}-night average {hm(avg(s.nights.map(asleep)))} asleep</p>
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
    <section className="card span2 body">
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
