// Browse: search, categories, metric lists, metric detail, all records.
import { useEffect, useState } from "react";
import { api, Sleep } from "../api";
import { Hypnogram, STAGES } from "../components/charts";
import { Link } from "../router";
import { conv, useHealth } from "./ctx";
import { BrowseMetric, CATEGORIES, cat, CatIcon, Chevron, fmtNum, hm, SLEEP, unitLabel, useApi, useBrowse, when, DEFAULT_PINNED } from "./meta";
import { Range, RangeChart, rangeLabel, Segmented, today, useRange } from "./range";
import { LargeTitle, NavBar } from "./ui";
import { Pager } from "./Summary";

function MetricRow({ m }: { m: BrowseMetric }) {
  const { units } = useHealth();
  const c = cat(m.category);
  const v = m.latest_value != null ? Number(m.latest_value) : NaN;
  const [cv, u] = conv(v, m.unit, units);
  return (
    <Link to={`/health/metric/${encodeURIComponent(m.metric)}`} className="row metricrow">
      <span className="row-icon" style={{ color: c.color }}><CatIcon k={m.category} /></span>
      <span className="row-label"><b>{m.name}</b><small className="muted">{when(m.latest_at)}</small></span>
      <span className="row-value num">{isNaN(cv) ? "" : <>{fmtNum(cv, m.metric)} <small>{u || unitLabel(m)}</small></>}</span>
      <Chevron />
    </Link>
  );
}

export default function Browse() {
  const b = useBrowse();
  const [q, setQ] = useState("");
  const ms = b.data?.metrics ?? [];
  const hits = q ? ms.filter((m) => m.name.toLowerCase().includes(q.toLowerCase())) : [];
  return (
    <div className="page">
      <LargeTitle title="Health" />
      <input className="search" type="search" placeholder="Search" value={q} onChange={(e) => setQ(e.target.value)} aria-label="Search health data" />
      {q ? (
        <div className="group">{hits.map((m) => <MetricRow key={m.metric} m={m} />)}{!hits.length && <p className="muted pad">No matches.</p>}</div>
      ) : (
        <>
          <p className="group-title">Health Categories</p>
          <div className="group browse-cats">
            {CATEGORIES.filter((c) => ms.some((m) => m.category === c.key)).map((c) => (
              <Link key={c.key} to={`/health/${c.key}`} className="row">
                <span className="row-icon" style={{ color: c.color }}><CatIcon k={c.key} /></span>
                <span className="row-label" style={{ color: c.color }}><b>{c.name}</b></span>
                <span className="row-value muted">{ms.filter((m) => m.category === c.key).length}</span>
                <Chevron />
              </Link>
            ))}
          </div>
          {b.data && !ms.length && <p className="muted">No data yet. <Link to="/settings/import">Import health data</Link>.</p>}
        </>
      )}
    </div>
  );
}

export function BrowseCategory({ k }: { k: string }) {
  const b = useBrowse();
  const c = cat(k);
  const ms = (b.data?.metrics ?? []).filter((m) => m.category === k);
  return (
    <div className="page">
      <NavBar to="/health" />
      <LargeTitle title={c.name} avatar={false} />
      <div className="group">{ms.map((m) => <MetricRow key={m.metric} m={m} />)}</div>
    </div>
  );
}

export function MetricDetail({ metric }: { metric: string }) {
  const { units } = useHealth();
  const b = useBrowse();
  const m = b.data?.metrics.find((x) => x.metric === metric);
  const [range, setRange] = useState<Range>("W");
  const [offset, setOffset] = useState(0);
  const [sel, setSel] = useState<number | null>(null);
  const [pins, setPins] = useState<string[] | null>(null);
  useEffect(() => { api<{ pinned: string[] }>("/pinned").then((r) => setPins(r.pinned.length ? r.pinned : DEFAULT_PINNED), () => setPins([])); }, []);
  const anchor = b.data ? (m?.last?.slice(0, 10) ?? today()) : null;
  const d = useRange(metric, range, anchor, offset);
  useEffect(() => setSel(null), [range, offset]);
  if (b.data && !m) return <div className="page"><NavBar to="/health" /><p className="muted">No data for this metric.</p></div>;
  const c = cat(m?.category ?? "other");
  const bk = sel !== null ? d?.buckets[sel] : null;
  const val = bk ? bk.value : d?.summary;
  const [v, u] = conv(val ?? 0, d?.unit ?? m?.unit ?? null, units);
  const pinned = pins?.includes(metric);
  const toggle = async () => {
    if (!pins) return;
    const next = pinned ? pins.filter((p) => p !== metric) : [...pins, metric];
    setPins(next);
    await api("/pinned", { method: "PUT", body: { pinned: next } }).catch(() => {});
  };
  return (
    <div className="page detail">
      <NavBar to={`/health/${m?.category ?? ""}`} title={m?.name} />
      <Segmented value={range} options={["D", "W", "M", "6M", "Y"]} onChange={(r) => { setRange(r); setOffset(0); }} />
      {metric === SLEEP && range === "D" ? (
        <SleepDayCard anchor={anchor} offset={offset} setOffset={setOffset} color={c.color} />
      ) : (
      <section className="card chartcard">
        <p className="eyebrow">{metric === SLEEP ? "TIME ASLEEP" : undefined}</p>
        <p className="big xl num" style={{ color: c.color }}>
          {val == null ? "--" : metric === SLEEP ? hm(v) : fmtNum(v, metric)}
          <small>{metric === SLEEP ? "" : (u || (m ? unitLabel(m) : ""))}</small>
        </p>
        <p className="muted small">{bk ? bk.title : (d ? rangeLabel(range, d) : "…")}</p>
        <RangeChart buckets={d?.buckets ?? []} color={c.color} height={220} selected={sel} onSelect={setSel} onSwipe={(dir) => setOffset(Math.max(0, offset + dir))} dots={d?.agg !== "sum"} />
        <Pager offset={offset} setOffset={setOffset} />
      </section>
      )}
      <p className="group-title">Options</p>
      <div className="group">
        <button type="button" className="row" onClick={toggle} disabled={!pins}>
          <span className="row-label">{pinned ? "Remove from Pinned" : "Add to Pinned"}</span>
          <span className={pinned ? "pinmark on" : "pinmark"} aria-hidden>{pinned ? "−" : "+"}</span>
        </button>
        <Link to={`/health/metric/${encodeURIComponent(metric)}/all`} className="row"><span className="row-label">Show All Data</span><span className="row-value muted num">{m?.count.toLocaleString()}</span><Chevron /></Link>
        <div className="row static"><span className="row-label">Data Sources</span><span className="row-value muted">{m?.count ? "Imported" : "—"}</span></div>
        <div className="row static"><span className="row-label">Unit</span><span className="row-value muted">{u || (m ? unitLabel(m) : "") || "—"}</span></div>
      </div>
      <p className="group-foot">Pinned topics will appear at the top of Summary.</p>
    </div>
  );
}

function SleepDayCard({ anchor, offset, setOffset, color }: { anchor: string | null; offset: number; setOffset: (n: number) => void; color: string }) {
  // end date shifted by pager offset
  const end = anchor ? (() => {
    const d = new Date(anchor + "T12:00:00");
    d.setDate(d.getDate() - offset);
    return d.toISOString().slice(0, 10);
  })() : null;
  const s = useApi<Sleep>(end ? `/dashboard/sleep?end=${end}&days=1` : null);
  const night = s.data?.nights?.[0];
  const asleep = night?.asleep_min ?? 0;
  const hrs = Math.floor(asleep / 60);
  const dateLabel = end ? new Date(end + "T12:00:00").toLocaleDateString([], { month: "short", day: "numeric", year: "numeric" }) : "…";
  return (
    <section className="card chartcard sleep-day">
      <p className="eyebrow">TIME ASLEEP</p>
      <p className="big xl num" style={{ color }}>{night ? hrs : "--"}<small>{night ? "hr" : ""}</small>
        {night && asleep % 60 >= 1 ? <> {Math.round(asleep % 60)}<small>min</small></> : null}
      </p>
      <p className="muted small">{dateLabel}</p>
      {s.data?.last_night?.length ? <Hypnogram segments={s.data.last_night} /> : <p className="muted">No sleep samples for this night.</p>}
      {night && (
        <div className="stage-totals">
          {STAGES.map((st) => {
            const key = { Awake: "awake_min", AsleepREM: "rem_min", AsleepCore: "core_min", AsleepDeep: "deep_min" }[st.key] as keyof NonNullable<typeof night>;
            const v = Number(night[key] ?? 0);
            return <div key={st.key}><i style={{ background: st.color }} /><span>{st.label}</span><b className="num">{hm(v)}</b></div>;
          })}
        </div>
      )}
      <Pager offset={offset} setOffset={setOffset} />
    </section>
  );
}


type Rec = { id: number; value: string | null; unit: string | null; start: string | null; end: string | null; source: string | null };

export function AllRecords({ metric }: { metric: string }) {
  const { units } = useHealth();
  const b = useBrowse();
  const m = b.data?.metrics.find((x) => x.metric === metric);
  const [recs, setRecs] = useState<Rec[]>([]);
  const [next, setNext] = useState<number | null | undefined>(undefined);
  const load = async (before?: number | null) => {
    const r = await api<{ records: Rec[]; next: number | null }>(`/records?metric=${encodeURIComponent(metric)}&limit=50${before ? `&before=${before}` : ""}`);
    setRecs((x) => (before ? [...x, ...r.records] : r.records));
    setNext(r.next);
  };
  useEffect(() => { load(); }, [metric]); // eslint-disable-line react-hooks/exhaustive-deps
  return (
    <div className="page">
      <NavBar to={`/health/metric/${encodeURIComponent(metric)}`} title="All Recorded Data" sub={m?.name} />
      <div className="group">
        {recs.map((r) => {
          const [v, u] = conv(Number(r.value), r.unit, units);
          return (
            <div key={r.id} className="row static">
              <span className="row-label"><b className="num">{r.value == null || isNaN(v) ? r.value : `${fmtNum(v, metric)} ${u}`}</b><small className="muted">{r.source ?? "Unknown source"}</small></span>
              <span className="row-value muted small">{r.start?.replace("T", " ").slice(0, 16)}</span>
            </div>
          );
        })}
      </div>
      {next && <div className="center"><button className="pill" onClick={() => load(next)}>Load more</button></div>}
      {next === null && <p className="muted center small">End of records.</p>}
    </div>
  );
}
