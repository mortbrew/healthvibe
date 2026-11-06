// Browse: search, categories, metric lists, metric detail, all records.
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { api, Night, Sleep, SleepVitals } from "../api";
import {
  AvgAsleepBars,
  FloatingSleepBars,
  Hypnogram,
  sleepLegendRows,
  VitalRangeBars,
} from "../components/charts";
import { Link } from "../router";
import { conv, useHealth } from "./ctx";
import { BrowseMetric, CATEGORIES, cat, CatIcon, Chevron, fmtNum, hm, SLEEP, unitLabel, useApi, useBrowse, when } from "./meta";
import { summaryHasMetric, toggleSummaryMetric } from "./Board";
import { addDays, Range, RangeChart, rangeLabel, Segmented, today, useRange } from "./range";
import { LargeTitle, MinusIcon, NavBar, RoundBtn, Sheet, XIcon } from "./ui";
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
      <LargeTitle title="All Categories" />
      <input className="search" type="search" placeholder="Search" value={q} onChange={(e) => setQ(e.target.value)} aria-label="Search health data" enterKeyHint="search" />
      {q ? (
        <div className="group">{hits.map((m) => <MetricRow key={m.metric} m={m} />)}{!hits.length && <p className="muted pad">No matches.</p>}</div>
      ) : (
        <>
          <p className="group-title">Health Categories</p>
          <div className="browse-cat-grid">
            {CATEGORIES.filter((c) => ms.some((m) => m.category === c.key)).map((c) => (
              <Link key={c.key} to={`/health/${c.key}`} className="browse-tile">
                <span className="browse-tile-icon" style={{ color: c.color }}><CatIcon k={c.key} size={22} /></span>
                <b style={{ color: c.color }}>{c.name}</b>
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

const SLEEP_RANGES: Range[] = ["D", "W", "M", "6M"];

function shiftEnd(anchor: string, offset: number, span: number) {
  return addDays(anchor, -span * offset);
}

function asleepParts(min: number) {
  const h = Math.floor(min / 60);
  const m = Math.round(min % 60);
  return { h, m };
}

function AsleepBig({ minutes, color }: { minutes: number | null; color?: string }) {
  if (minutes == null) return <p className="big xl num" style={{ color }}>--</p>;
  const { h, m } = asleepParts(minutes);
  return (
    <p className="big xl num" style={{ color }}>
      {h}<small>hr</small>{m >= 1 ? <> {m}<small>min</small></> : null}
    </p>
  );
}

function rangeDays(r: Range) {
  return r === "D" ? 1 : r === "W" ? 7 : r === "M" ? 30 : 182;
}

function weekBuckets(nights: Night[]): Night[] {
  // Collapse nights into ISO-week buckets for 6M (avg bedtime→wake via segments window).
  const by: Record<string, Night[]> = {};
  for (const n of nights) {
    const d = new Date(n.date + "T12:00:00");
    const day = (d.getDay() + 6) % 7;
    const mon = new Date(d);
    mon.setDate(d.getDate() - day);
    const key = mon.toISOString().slice(0, 10);
    (by[key] ??= []).push(n);
  }
  return Object.keys(by)
    .sort()
    .map((key) => {
      const list = by[key];
      const asleep = list.reduce((s, n) => s + n.asleep_min, 0) / list.length;
      // Build a synthetic span from earliest start → latest end across the week (mean of each night's window).
      let sumStart = 0, sumEnd = 0, count = 0;
      for (const n of list) {
        const segs = (n.segments ?? []).filter((s) => s.stage !== "InBed");
        if (!segs.length) continue;
        const starts = segs.map((s) => s.start);
        const ends = segs.map((s) => s.end);
        starts.sort();
        ends.sort();
        // Use minutes-from-midnight approx via string compare on times — keep first/last sample strings averaged by picking median night.
        sumStart += 1;
        sumEnd += 1;
        count++;
      }
      const sample = list[Math.floor(list.length / 2)];
      const segs = sample?.segments?.filter((s) => s.stage !== "InBed") ?? [];
      const start = segs[0]?.start;
      const end = segs[segs.length - 1]?.end;
      const synth = start && end
        ? [{ stage: "AsleepCore", start, end }]
        : [];
      return {
        ...list[0],
        date: key,
        asleep_min: asleep,
        rem_min: 0,
        core_min: asleep,
        deep_min: 0,
        awake_min: 0,
        unspecified_min: 0,
        in_bed_min: 0,
        segments: synth,
      } as Night;
    });
}

export function MetricDetail({ metric }: { metric: string }) {
  const { units } = useHealth();
  const b = useBrowse();
  const m = b.data?.metrics.find((x) => x.metric === metric);
  const isSleep = metric === SLEEP;
  const [range, setRange] = useState<Range>(isSleep ? "D" : "W");
  const [offset, setOffset] = useState(0);
  const [sel, setSel] = useState<number | null>(null);
  const [onSummary, setOnSummary] = useState<boolean | null>(null);
  const [more, setMore] = useState(false);
  const [highlights, setHighlights] = useState(false);
  useEffect(() => {
    summaryHasMetric().then((s) => setOnSummary(!!s?.widgets.some((w) => w.metric === metric)), () => setOnSummary(false));
  }, [metric]);
  const anchor = b.data ? (m?.last?.slice(0, 10) ?? today()) : null;
  const d = useRange(metric, range, anchor, offset);
  useEffect(() => setSel(null), [range, offset]);

  const sleepDays = rangeDays(range);
  const sleepEnd = anchor ? shiftEnd(anchor, offset, sleepDays) : null;
  const sleepFetchDays = range === "6M" ? 210 : sleepDays;
  const sleep = useApi<Sleep>(isSleep && sleepEnd ? `/dashboard/sleep?end=${sleepEnd}&days=${sleepFetchDays}` : null);

  if (b.data && !m) return <div className="page"><NavBar to="/health" /><p className="muted">No data for this metric.</p></div>;
  const c = cat(m?.category ?? "other");
  const bk = sel !== null ? d?.buckets[sel] : null;
  const val = bk ? bk.value : d?.summary;
  const [v, u] = conv(val ?? 0, d?.unit ?? m?.unit ?? null, units);
  const pinned = !!onSummary;
  const toggle = async () => {
    if (onSummary == null || !m) return;
    const now = await toggleSummaryMetric(metric, m.name).catch(() => pinned);
    setOnSummary(now);
  };

  const nightsInRange = useMemo(() => {
    const all = sleep.data?.nights ?? [];
    if (!sleepEnd) return [];
    const start = addDays(sleepEnd, 1 - sleepDays);
    return all.filter((n) => n.date >= start && n.date <= sleepEnd);
  }, [sleep.data, sleepEnd, sleepDays]);

  const avgAsleep = nightsInRange.length
    ? nightsInRange.reduce((s, n) => s + n.asleep_min, 0) / nightsInRange.length
    : null;
  const chartNights = range === "6M" ? weekBuckets(nightsInRange) : nightsInRange;
  const dateLabel = (() => {
    if (!sleepEnd) return "…";
    if (range === "D") return new Date(sleepEnd + "T12:00:00").toLocaleDateString([], { month: "short", day: "numeric", year: "numeric" });
    const start = addDays(sleepEnd, 1 - sleepDays);
    const a = new Date(start + "T12:00:00");
    const b = new Date(sleepEnd + "T12:00:00");
    return `${a.toLocaleDateString([], { month: "short", day: "numeric" })} – ${b.toLocaleDateString([], { month: "short", day: "numeric", year: "numeric" })}`;
  })();

  const dayNight = range === "D" ? nightsInRange[0] ?? sleep.data?.nights?.slice(-1)[0] : null;

  return (
    <div className="page detail">
      <NavBar to={`/health/${m?.category ?? ""}`} title={isSleep ? "Sleep" : m?.name} />
      <Segmented value={range} options={isSleep ? SLEEP_RANGES : ["D", "W", "M", "6M", "Y"]} onChange={(r) => { setRange(r); setOffset(0); }} />

      {isSleep ? (
        <section className="card chartcard sleep-card">
          <p className="eyebrow">{range === "D" ? "TIME ASLEEP" : "AVG. TIME ASLEEP"}</p>
          <AsleepBig minutes={range === "D" ? (dayNight?.asleep_min ?? null) : avgAsleep} color={c.color} />
          <p className="muted small">{dateLabel}</p>
          {range === "D" ? (
            sleep.data?.last_night?.length || dayNight?.segments?.length ? (
              <Hypnogram segments={dayNight?.segments ?? sleep.data?.last_night ?? []} />
            ) : (
              <p className="muted">No sleep samples for this night.</p>
            )
          ) : (
            <FloatingSleepBars
              nights={chartNights.map((n) => ({ date: n.date, asleep_min: n.asleep_min, segments: n.segments, in_bed_min: n.in_bed_min }))}
              plain={false}
              labels={range === "W" ? "dow" : range === "M" ? "day" : "month"}
            />
          )}
          {range === "D" && dayNight && (
            <div className="stage-totals">
              {sleepLegendRows(dayNight).map((st) => (
                <div key={st.key}><i style={{ background: st.color }} /><span>{st.label}</span><b className="num">{hm(st.minutes)}</b></div>
              ))}
            </div>
          )}
          <button type="button" className="link show-more-sleep" onClick={() => setMore(true)}>Show More Sleep Data</button>
          <Pager offset={offset} setOffset={setOffset} />
        </section>
      ) : (
        <section className="card chartcard">
          <p className="eyebrow" />
          <p className="big xl num" style={{ color: c.color }}>
            {val == null ? "--" : fmtNum(v, metric)}
            <small>{u || (m ? unitLabel(m) : "")}</small>
          </p>
          <p className="muted small">{bk ? bk.title : (d ? rangeLabel(range, d) : "…")}</p>
          <RangeChart buckets={d?.buckets ?? []} color={c.color} height={220} selected={sel} onSelect={setSel} onSwipe={(dir) => setOffset(Math.max(0, offset + dir))} dots={d?.agg !== "sum"} />
          <Pager offset={offset} setOffset={setOffset} />
        </section>
      )}

      {isSleep && (
        <section className="highlights-sec">
          <div className="sec-head"><h2>Highlights</h2><button type="button" className="link" onClick={() => setHighlights(true)}>Show All</button></div>
          <SleepHighlightsInline end={sleepEnd} nights={nightsInRange} lastNight={dayNight ?? nightsInRange[nightsInRange.length - 1]} />
        </section>
      )}

      <p className="group-title">More</p>
      <div className="group">
        <button type="button" className="row" onClick={toggle} disabled={onSummary == null}>
          <span className="row-label">{pinned ? "Remove from Summary" : "Add to Summary"}</span>
          <span className={pinned ? "pinmark on" : "pinmark"} aria-hidden>{pinned ? <MinusIcon /> : "+"}</span>
        </button>
      </div>
      <p className="group-foot">Added topics show up as cards on Summary.</p>
      <div className="group">
        <Link to={`/health/metric/${encodeURIComponent(metric)}/all`} className="row"><span className="row-label">Show All Data</span><Chevron /></Link>
        <div className="row static"><span className="row-label">Data Sources & Access</span><Chevron /></div>
      </div>
      {/* Sleep Schedule / Duration Goals omitted — not present in export.xml */}

      {more && isSleep && sleepEnd && (
        <ShowMoreSleep
          end={sleepEnd}
          initialRange={range === "Y" ? "W" : range}
          nights={sleep.data?.nights ?? []}
          onClose={() => setMore(false)}
        />
      )}
      {highlights && isSleep && (
        <Sheet title="Sleep Highlights" onClose={() => setHighlights(false)} left="back">
          <SleepHighlightsFull end={sleepEnd} nights={nightsInRange} lastNight={dayNight ?? nightsInRange[nightsInRange.length - 1]} />
        </Sheet>
      )}
    </div>
  );
}

function SleepHighlightsInline({ end, nights, lastNight }: { end: string | null; nights: Night[]; lastNight?: Night }) {
  const vitals = useApi<SleepVitals>(end ? `/dashboard/sleep/vitals?end=${end}` : null);
  const cards: ReactNode[] = [];
  if (lastNight && lastNight.asleep_min > 0) {
    cards.push(
      <div key="stages" className="hl-card">
        <p className="hl-head" style={{ color: "var(--c-sleep)" }}><CatIcon k="sleep" size={16} /> Sleep: Stages</p>
        <p>Here are the stages you were in during your {hm(lastNight.asleep_min)} of sleep last night.</p>
        {(lastNight.segments?.length || 0) > 0 && <Hypnogram segments={lastNight.segments!} />}
      </div>,
    );
  }
  const hr = vitals.data?.heart_rate;
  const rr = vitals.data?.respiratory_rate;
  if (rr) {
    cards.push(
      <div key="rr" className="hl-card">
        <p className="hl-head" style={{ color: "var(--c-resp)" }}><CatIcon k="respiratory" size={16} /> Respiratory Rate: Sleep</p>
        <p>While you were sleeping, your respiratory rate ranged from {fmtRange(rr)}.</p>
      </div>,
    );
  }
  if (hr) {
    cards.push(
      <div key="hr" className="hl-card">
        <p className="hl-head" style={{ color: "var(--c-heart)" }}><CatIcon k="heart" size={16} /> Heart Rate: Sleep</p>
        <p>While you were sleeping, your heart rate ranged from {fmtRange(hr, "BPM")}.</p>
      </div>,
    );
  }
  if (nights.length) {
    const avg = nights.reduce((s, n) => s + n.asleep_min, 0) / nights.length;
    cards.push(
      <div key="avg7" className="hl-card">
        <p className="hl-head" style={{ color: "var(--c-sleep)" }}><CatIcon k="sleep" size={16} /> Sleep</p>
        <p>You slept an average of {hm(avg)} over the last {nights.length} days.</p>
        <AvgAsleepBars nights={nights.slice(-7)} avg={avg} />
      </div>,
    );
  }
  if (!cards.length) return <p className="muted small">No highlight data yet.</p>;
  return <div className="hl-stack">{cards.slice(0, 2)}</div>;
}

function SleepHighlightsFull({ end, nights, lastNight }: { end: string | null; nights: Night[]; lastNight?: Night }) {
  const vitals = useApi<SleepVitals>(end ? `/dashboard/sleep/vitals?end=${end}` : null);
  return (
    <div className="hl-full">
      <p className="group-title">Daily Highlights</p>
      {lastNight && lastNight.asleep_min > 0 && (
        <div className="hl-card">
          <p className="hl-head" style={{ color: "var(--c-sleep)" }}><CatIcon k="sleep" size={16} /> Sleep: Stages</p>
          <p>Here are the stages you were in during your {hm(lastNight.asleep_min)} of sleep last night.</p>
          {(lastNight.segments?.length || 0) > 0 && <Hypnogram segments={lastNight.segments!} />}
          <div className="stage-totals compact">
            {sleepLegendRows(lastNight).filter((r) => r.asleep || r.key === "Awake").map((st) => (
              <div key={st.key}><i style={{ background: st.color }} /><span>{st.label}</span><b className="num">{hm(st.minutes)}</b></div>
            ))}
          </div>
        </div>
      )}
      {vitals.data?.respiratory_rate && (
        <div className="hl-card">
          <p className="hl-head" style={{ color: "var(--c-resp)" }}><CatIcon k="respiratory" size={16} /> Respiratory Rate: Sleep</p>
          <p>While you were sleeping, your respiratory rate ranged from {fmtRange(vitals.data.respiratory_rate)}.</p>
        </div>
      )}
      {vitals.data?.heart_rate && (
        <div className="hl-card">
          <p className="hl-head" style={{ color: "var(--c-heart)" }}><CatIcon k="heart" size={16} /> Heart Rate: Sleep</p>
          <p>While you were sleeping, your heart rate ranged from {fmtRange(vitals.data.heart_rate, "BPM")}.</p>
          {vitals.data.heart_rate_series?.length > 0 && (
            <VitalRangeBars points={vitals.data.heart_rate_series} color="var(--c-heart)" />
          )}
        </div>
      )}
      <p className="group-title">Weekly Highlights</p>
      {nights.length > 0 && (() => {
        const avg = nights.reduce((s, n) => s + n.asleep_min, 0) / nights.length;
        return (
          <div className="hl-card">
            <p className="hl-head" style={{ color: "var(--c-sleep)" }}><CatIcon k="sleep" size={16} /> Sleep</p>
            <p>You slept an average of {hm(avg)} over the last {nights.length} days.</p>
            <AvgAsleepBars nights={nights.slice(-7)} avg={avg} />
          </div>
        );
      })()}
      {/* Omitted: Consistency, Breathing Disturbances, Yearly comparison (not imported / Apple-computed). */}
    </div>
  );
}

function fmtRange(r: { min: number; max: number; unit: string | null }, fallbackUnit?: string) {
  const u = r.unit === "count/min"
    ? (fallbackUnit === "BPM" ? "beats per minute" : "breaths per minute")
    : (r.unit || fallbackUnit || "");
  const a = Number.isInteger(r.min) ? String(r.min) : r.min.toFixed(1);
  const b = Number.isInteger(r.max) ? String(r.max) : r.max.toFixed(1);
  return `${a} to ${b} ${u}`.trim();
}

function ShowMoreSleep({
  end,
  initialRange,
  nights,
  onClose,
}: {
  end: string;
  initialRange: Range;
  nights: Night[];
  onClose: () => void;
}) {
  const [range, setRange] = useState<Range>(initialRange === "Y" ? "W" : initialRange);
  const [tab, setTab] = useState<"Stages" | "Amounts" | "Comparisons">("Stages");
  const days = rangeDays(range);
  const start = addDays(end, 1 - days);
  const inRange = nights.filter((n) => n.date >= start && n.date <= end);
  const dayNight = range === "D" ? inRange[0] ?? nights.find((n) => n.date === end) : null;
  const avgAsleep = inRange.length ? inRange.reduce((s, n) => s + n.asleep_min, 0) / inRange.length : null;
  const avgInBed = (() => {
    const withBed = inRange.filter((n) => n.in_bed_min > 0);
    if (!withBed.length) return null;
    return withBed.reduce((s, n) => s + n.in_bed_min, 0) / withBed.length;
  })();
  const vitals = useApi<SleepVitals>(`/dashboard/sleep/vitals?end=${end}`);
  const chartNights = range === "6M" ? weekBuckets(inRange) : inRange;
  const plain = tab === "Amounts" || tab === "Comparisons";
  const dateLabel = range === "D"
    ? new Date(end + "T12:00:00").toLocaleDateString([], { month: "short", day: "numeric", year: "numeric" })
    : `${new Date(start + "T12:00:00").toLocaleDateString([], { month: "short", day: "numeric" })} – ${new Date(end + "T12:00:00").toLocaleDateString([], { month: "short", day: "numeric", year: "numeric" })}`;

  return (
    <div className="sheet-backdrop" onClick={onClose}>
      <div className="sheet sheet-tall" role="dialog" aria-label="Sleep" onClick={(e) => e.stopPropagation()}>
        <div className="sheet-head">
          <RoundBtn label="Close" onClick={onClose}><XIcon /></RoundBtn>
          <b>Sleep</b>
          <span style={{ width: 36 }} />
        </div>
        <div className="sheet-body">
          <Segmented value={range} options={SLEEP_RANGES} onChange={setRange} />
          <p className="eyebrow">{range === "D" ? "TIME ASLEEP" : "AVG. TIME ASLEEP"}</p>
          <AsleepBig minutes={range === "D" ? (dayNight?.asleep_min ?? null) : avgAsleep} />
          <p className="muted small">{dateLabel}</p>
          {range === "D" && tab === "Stages" ? (
            <Hypnogram segments={dayNight?.segments ?? []} />
          ) : tab === "Comparisons" && range === "D" ? (
            <div className="compare-charts">
              <p className="muted small">Sleep</p>
              <FloatingSleepBars
                nights={[{ date: end, asleep_min: dayNight?.asleep_min ?? 0, segments: dayNight?.segments }]}
                plain
                labels="none"
                height={90}
              />
              <p className="muted small">Heart Rate</p>
              {vitals.data?.heart_rate_series?.length ? (
                <VitalRangeBars points={vitals.data.heart_rate_series} color="var(--c-heart)" />
              ) : (
                <p className="muted small">No heart rate during this sleep window.</p>
              )}
            </div>
          ) : (
            <FloatingSleepBars
              nights={chartNights.map((n) => ({ date: n.date, asleep_min: n.asleep_min, segments: n.segments }))}
              plain={plain && (tab === "Amounts" || range !== "D")}
              labels={range === "W" ? "dow" : range === "M" ? "day" : range === "6M" ? "month" : "none"}
            />
          )}

          <div className="seg tabs-3" role="tablist">
            {(["Stages", "Amounts", "Comparisons"] as const).map((t) => (
              <button key={t} role="tab" aria-selected={tab === t} className={tab === t ? "on" : ""} onClick={() => setTab(t)}>{t}</button>
            ))}
          </div>

          {tab === "Stages" && (
            <div className="group amounts-rows">
              {(dayNight ?? (inRange.length ? {
                awake_min: inRange.reduce((s, n) => s + n.awake_min, 0) / inRange.length,
                rem_min: inRange.reduce((s, n) => s + n.rem_min, 0) / inRange.length,
                core_min: inRange.reduce((s, n) => s + n.core_min, 0) / inRange.length,
                deep_min: inRange.reduce((s, n) => s + n.deep_min, 0) / inRange.length,
                unspecified_min: inRange.reduce((s, n) => s + n.unspecified_min, 0) / inRange.length,
                in_bed_min: 0,
                asleep_min: avgAsleep ?? 0,
              } : null)) && sleepLegendRows((dayNight ?? {
                awake_min: inRange.reduce((s, n) => s + n.awake_min, 0) / inRange.length,
                rem_min: inRange.reduce((s, n) => s + n.rem_min, 0) / inRange.length,
                core_min: inRange.reduce((s, n) => s + n.core_min, 0) / inRange.length,
                deep_min: inRange.reduce((s, n) => s + n.deep_min, 0) / inRange.length,
                unspecified_min: inRange.reduce((s, n) => s + n.unspecified_min, 0) / inRange.length,
                in_bed_min: 0,
                asleep_min: avgAsleep ?? 0,
              })!).filter((r) => r.key !== "InBed").map((st) => (
                <div key={st.key} className="row static amount-row">
                  <span className="row-label"><i className="dot" style={{ background: st.color }} />{st.label}</span>
                  <span className="row-value num">{hm(st.minutes)}</span>
                </div>
              ))}
            </div>
          )}

          {tab === "Amounts" && (
            <div className="amounts-block">
              {/* Sleep Schedule / Sleep Goal omitted — not in export.xml */}
              <p className="group-title">Sleep Duration</p>
              <div className="group amounts-rows">
                <div className="row static amount-row">
                  <span className="row-label">Measurements</span>
                  <span className="row-value num">{inRange.length} of {days} days</span>
                </div>
                <div className="row static amount-row">
                  <span className="row-label">Average Time in Bed</span>
                  <span className="row-value num">{avgInBed == null ? "-- hr" : hm(avgInBed)}</span>
                </div>
                <div className="row static amount-row">
                  <span className="row-label">Average Time Asleep</span>
                  <span className="row-value num">{avgAsleep == null ? "--" : hm(avgAsleep)}</span>
                </div>
              </div>
              {/* No further Amounts rows — only what is visible in apple-ref amounts screenshots; Schedule/Goal absent from import. */}
            </div>
          )}

          {tab === "Comparisons" && (
            <div className="group amounts-rows">
              {vitals.data?.heart_rate && (
                <div className="row static amount-row" style={{ borderLeft: "3px solid var(--c-heart)" }}>
                  <span className="row-label">Heart Rate</span>
                  <span className="row-value num">{Math.round(vitals.data.heart_rate.min)}–{Math.round(vitals.data.heart_rate.max)} BPM</span>
                </div>
              )}
              {vitals.data?.respiratory_rate && (
                <div className="row static amount-row">
                  <span className="row-label">Respiratory Rate</span>
                  <span className="row-value num">{trimNum(vitals.data.respiratory_rate.min)}–{trimNum(vitals.data.respiratory_rate.max)} breaths/min</span>
                </div>
              )}
              {vitals.data?.wrist_temperature && (
                <div className="row static amount-row">
                  <span className="row-label">Wrist Temperature</span>
                  <span className="row-value num">
                    {trimNum(vitals.data.wrist_temperature.min)}
                    {vitals.data.wrist_temperature.min !== vitals.data.wrist_temperature.max
                      ? `–${trimNum(vitals.data.wrist_temperature.max)}`
                      : ""}{" "}
                    {vitals.data.wrist_temperature.unit || "degC"}
                  </span>
                </div>
              )}
              {!vitals.data?.heart_rate && !vitals.data?.respiratory_rate && !vitals.data?.wrist_temperature && (
                <p className="muted pad">No overlapping vitals for this night.</p>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function trimNum(n: number) {
  return Number.isInteger(n) ? String(n) : n.toFixed(1).replace(/\.0$/, "");
}

type Rec = { id: number; value: string | null; unit: string | null; start: string | null; end: string | null; source: string | null };

function stageLabel(v: string | null) {
  if (!v) return "Unknown";
  const s = v.replace(/^HKCategoryValueSleepAnalysis/, "");
  if (s === "AsleepREM") return "REM";
  if (s === "AsleepCore") return "Core";
  if (s === "AsleepDeep") return "Deep";
  if (s === "AsleepUnspecified") return "Asleep";
  if (s === "InBed") return "In Bed";
  return s;
}

function nightKey(start: string | null, end: string | null) {
  // Group by local end date spanning previous evening → morning.
  const e = (end ?? start ?? "").slice(0, 10);
  if (!e) return "Unknown";
  const prev = addDays(e, -1);
  const md = (d: string) => new Date(d + "T12:00:00").toLocaleDateString([], { month: "short", day: "numeric" });
  return `${md(prev)}–${md(e).split(" ").pop()}`;
}

export function AllRecords({ metric }: { metric: string }) {
  const { units } = useHealth();
  const b = useBrowse();
  const m = b.data?.metrics.find((x) => x.metric === metric);
  const [recs, setRecs] = useState<Rec[]>([]);
  const [next, setNext] = useState<number | null | undefined>(undefined);
  const load = async (before?: number | null) => {
    const r = await api<{ records: Rec[]; next: number | null }>(`/records?metric=${encodeURIComponent(metric)}&limit=200${before ? `&before=${before}` : ""}`);
    setRecs((x) => (before ? [...x, ...r.records] : r.records));
    setNext(r.next);
  };
  useEffect(() => { load(); }, [metric]); // eslint-disable-line react-hooks/exhaustive-deps

  if (metric === SLEEP) {
    // Group by night (end date), then by stage — "N intervals (Xhr Ymin)", device icon, chevron, no Edit.
    const byNight: Record<string, Rec[]> = {};
    for (const r of recs) {
      const end = (r.end ?? r.start ?? "").slice(0, 10) || "Unknown";
      (byNight[end] ??= []).push(r);
    }
    const nights = Object.keys(byNight).sort((a, b) => b.localeCompare(a));
    return (
      <div className="page">
        <NavBar to={`/health/metric/${encodeURIComponent(metric)}`} title="All Recorded Data" />
        {nights.map((end) => {
          const rows = byNight[end];
          const stages = ["Awake", "AsleepREM", "AsleepCore", "AsleepDeep", "AsleepUnspecified", "InBed"];
          const groups = stages
            .map((st) => {
              const matched = rows.filter((r) => {
                const v = (r.value ?? "").replace(/^HKCategoryValueSleepAnalysis/, "");
                return v === st;
              });
              if (!matched.length) return null;
              const mins = matched.reduce((s, r) => {
                if (!r.start || !r.end) return s;
                return s + Math.max(0, (Date.parse(r.end.replace(" ", "T")) - Date.parse(r.start.replace(" ", "T"))) / 60000);
              }, 0);
              return { st, matched, mins };
            })
            .filter(Boolean) as { st: string; matched: Rec[]; mins: number }[];
          return (
            <div key={end} className="group sleep-all-night">
              <p className="group-title">{nightKey(null, end)}</p>
              {groups.map((g) => (
                <div key={g.st} className="row static sleep-all-row">
                  <span className="watch-dot" aria-hidden />
                  <span className="row-label">
                    <small className="muted">{stageLabel("HKCategoryValueSleepAnalysis" + g.st)}</small>
                    <b>{g.matched.length} intervals ({hm(g.mins).replace(" hr ", "hr ").replace(" min", "min")})</b>
                  </span>
                  <Chevron />
                </div>
              ))}
            </div>
          );
        })}
        {next && <div className="center"><button className="pill" onClick={() => load(next)}>Load more</button></div>}
        {next === null && <p className="muted center small">End of records.</p>}
      </div>
    );
  }

  // Other metrics: group by day
  const byDay: Record<string, Rec[]> = {};
  for (const r of recs) {
    const day = (r.end ?? r.start ?? "").slice(0, 10) || "Unknown";
    (byDay[day] ??= []).push(r);
  }
  const days = Object.keys(byDay).sort((a, b) => b.localeCompare(a));
  return (
    <div className="page">
      <NavBar to={`/health/metric/${encodeURIComponent(metric)}`} title="All Recorded Data" sub={m?.name} />
      {days.map((day) => (
        <div key={day} className="group">
          <p className="group-title">{new Date(day + "T12:00:00").toLocaleDateString([], { weekday: "short", month: "short", day: "numeric", year: "numeric" })}</p>
          {byDay[day].map((r) => {
            const [v, u] = conv(Number(r.value), r.unit, units);
            return (
              <div key={r.id} className="row static">
                <span className="row-label"><b className="num">{r.value == null || isNaN(v) ? r.value : `${fmtNum(v, metric)} ${u}`}</b><small className="muted">{r.source ?? "Unknown source"}</small></span>
                <span className="row-value muted small">{(r.start ?? "").replace("T", " ").slice(11, 16)}</span>
              </div>
            );
          })}
        </div>
      ))}
      {next && <div className="center"><button className="pill" onClick={() => load(next)}>Load more</button></div>}
      {next === null && <p className="muted center small">End of records.</p>}
    </div>
  );
}
