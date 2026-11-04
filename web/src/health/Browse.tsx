// Browse: search, categories, metric lists, metric detail, all records.
import { useEffect, useState } from "react";
import { api } from "../api";
import { Link } from "../router";
import { conv, useHealth } from "./ctx";
import { BrowseMetric, CATEGORIES, cat, CatIcon, Chevron, fmtNum, unitLabel, useBrowse, when, DEFAULT_PINNED } from "./meta";
import { Range, RangeChart, rangeLabel, Segmented, today, useRange } from "./range";
import { LargeTitle, NavBar } from "./ui";
import { Pager } from "./Summary";

function MetricRow({ m }: { m: BrowseMetric }) {
  const { units } = useHealth();
  const c = cat(m.category);
  const v = m.latest_value != null ? Number(m.latest_value) : NaN;
  const [cv, u] = conv(v, m.unit, units);
  return (
    <Link to={`/browse/metric/${m.metric}`} className="row metricrow">
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
      <LargeTitle title="Browse" />
      <input className="search" type="search" placeholder="Search" value={q} onChange={(e) => setQ(e.target.value)} aria-label="Search health data" />
      {q ? (
        <div className="group">{hits.map((m) => <MetricRow key={m.metric} m={m} />)}{!hits.length && <p className="muted pad">No matches.</p>}</div>
      ) : (
        <>
          <p className="group-title">Health Categories</p>
          <div className="group">
            {CATEGORIES.filter((c) => ms.some((m) => m.category === c.key)).map((c) => (
              <Link key={c.key} to={`/browse/${c.key}`} className="row">
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
      <NavBar to="/browse" />
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
  if (b.data && !m) return <div className="page"><NavBar to="/browse" /><p className="muted">No data for this metric.</p></div>;
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
  const head = bk ? bk.title.toUpperCase() : d?.agg === "sum" ? (range === "D" ? "TOTAL" : "DAILY AVERAGE") : "AVERAGE";
  return (
    <div className="page">
      <NavBar to={`/browse/${m?.category ?? ""}`} title={m?.name} right={<button className={pinned ? "pinbtn on" : "pinbtn"} onClick={toggle} aria-pressed={!!pinned}>{pinned ? "★ Pinned" : "☆ Pin"}</button>} />
      <Segmented value={range} options={["D", "W", "M", "6M", "Y"]} onChange={(r) => { setRange(r); setOffset(0); }} />
      <section className="card">
        <p className="eyebrow">{head}</p>
        <p className="big xl num" style={{ color: c.color }}>{val == null ? "--" : fmtNum(v, metric)}<small>{u || (m ? unitLabel(m) : "")}</small></p>
        <p className="muted small">{d ? rangeLabel(range, d) : "…"}</p>
        <RangeChart buckets={d?.buckets ?? []} color={c.color} height={220} selected={sel} onSelect={setSel} onSwipe={(dir) => setOffset(Math.max(0, offset + dir))} dots={d?.agg !== "sum"} />
        <Pager offset={offset} setOffset={setOffset} />
      </section>
      <div className="group">
        <Link to={`/browse/metric/${metric}/all`} className="row"><span className="row-label">Show All Data</span><span className="row-value muted num">{m?.count.toLocaleString()}</span><Chevron /></Link>
        {m && <div className="row static"><span className="row-label muted">First recorded</span><span className="row-value">{m.first?.slice(0, 10)}</span></div>}
      </div>
    </div>
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
      <NavBar to={`/browse/metric/${metric}`} title="All Recorded Data" sub={m?.name} />
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
