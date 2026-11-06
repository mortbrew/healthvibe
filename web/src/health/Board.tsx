// One dashboard surface for Summary and custom dashboards. Cards come from the
// shared widget list. A new board has none until the user adds one.
import { useEffect, useState } from "react";
import { api, Dashboard, explain, Widget } from "../api";
import { WidgetView } from "../pages/Dashboard";
import { BrowseMetric, useBrowse } from "./meta";
import { moveItem } from "./nav-layout";
import { useGripReorder } from "./reorder";
import { DownIcon, GripIcon, LargeTitle, MinusIcon, UpIcon, XIcon } from "./ui";

const newId = () => Math.random().toString(36).slice(2, 10);

const CATALOG: { kind: Widget["kind"]; label: string; blurb: string }[] = [
  { kind: "activity", label: "Activity", blurb: "Rings, Move, Exercise, Stand, and where the goal came from" },
  { kind: "sleep", label: "Sleep", blurb: "Imported sleep stages" },
  { kind: "heart", label: "Heart", blurb: "Resting heart rate and daily range" },
  { kind: "body", label: "Body", blurb: "Imported body measurements" },
];

function blank(kind: Widget["kind"], metric?: string, title?: string): Widget {
  return { id: newId(), kind, metric, title, days: kind === "body" ? 90 : 14, bucket: "day", size: 2 };
}

export function Board({ which }: { which: "summary" | number }) {
  const browse = useBrowse();
  const [boards, setBoards] = useState<Dashboard[] | null>(null);
  const [editing, setEditing] = useState(false);
  const [adding, setAdding] = useState(false);
  const [q, setQ] = useState("");
  const [err, setErr] = useState("");

  const load = () => {
    api<{ dashboards: Dashboard[] }>("/dashboards").then((r) => setBoards(r.dashboards), () => setErr("Couldn't load this dashboard."));
  };
  useEffect(load, []);

  const board = boards?.find((b) => (which === "summary" ? b.builtin === "summary" : b.id === which)) ?? null;
  const metrics = browse.data?.metrics ?? [];

  const save = async (widgets: Widget[]) => {
    if (!board) return;
    setErr("");
    try {
      const next = (await api<{ dashboard: Dashboard }>(`/dashboards/${board.id}`, { method: "PUT", body: { widgets } })).dashboard;
      setBoards((list) => list?.map((b) => (b.id === next.id ? next : b)) ?? [next]);
    } catch (e) {
      setErr(explain(e));
    }
  };

  const add = async (w: Widget) => {
    if (!board || board.widgets.length >= 40) return;
    await save([...board.widgets, w]);
    setAdding(false);
    setQ("");
  };
  const removeAt = (i: number) => board && save(board.widgets.filter((_, j) => j !== i));
  const move = (from: number, to: number) => board && save(moveItem(board.widgets, from, to));
  const grip = useGripReorder(move);

  if (!boards && err) return <div className="card empty"><p>{err}</p></div>;
  if (!boards) return <p className="muted">Loading…</p>;
  if (!board) return <div className="card empty"><p>That dashboard is not on this account.</p></div>;

  const title = board.builtin === "summary" ? "Summary" : board.name;
  return (
    <div className="page summary">
      <LargeTitle title={title} right={<button type="button" className="link" onClick={() => setEditing((v) => !v)}>{editing ? "Done" : "Edit"}</button>} />
      {err && <p className="muted">{err}</p>}
      {board.widgets.length === 0 && (
        <div className="card empty">
          <p>No cards yet.</p>
          <button type="button" className="pill" onClick={() => setAdding(true)}>Add Card</button>
        </div>
      )}
      <div className="board-cards">
        {board.widgets.map((w, i) => {
          const label = w.title || w.kind;
          return (
            <div key={w.id} className="board-card" {...(editing ? grip.rowProps(i) : {})}>
              {editing && (
                <div className="board-card-tools" role="group" aria-label={`Move or remove ${label}`}>
                  <button type="button" className="iconbtn grip" aria-label={`Drag to reorder ${label}`} {...grip.gripProps(i)}><GripIcon /></button>
                  <button type="button" className="iconbtn" onClick={() => move(i, i - 1)} disabled={i === 0} aria-label={`Move ${label} up`}><UpIcon /></button>
                  <button type="button" className="iconbtn" onClick={() => move(i, i + 1)} disabled={i === board.widgets.length - 1} aria-label={`Move ${label} down`}><DownIcon /></button>
                  <button type="button" className="iconbtn" onClick={() => removeAt(i)} aria-label={`Remove ${label}`}><MinusIcon /></button>
                </div>
              )}
              <WidgetView w={w} metrics={metrics} />
            </div>
          );
        })}
      </div>
      {board.widgets.length > 0 && <button type="button" className="pill fit-action" onClick={() => setAdding(true)}>Add Card</button>}
      {adding && (
        <div className="sheet-backdrop" onClick={() => setAdding(false)}>
          <div className="sheet" role="dialog" aria-label="Add Card" onClick={(e) => e.stopPropagation()}>
            <div className="sheet-head">
              <button type="button" className="roundbtn" aria-label="Close" onClick={() => setAdding(false)}><XIcon /></button>
              <b>Add Card</b>
              <span style={{ width: 38 }} />
            </div>
            <div className="sheet-body">
              <div className="group">
                {CATALOG.map((c) => (
                  <button key={c.kind} type="button" className="row" onClick={() => add(blank(c.kind))}>
                    <span className="row-label"><b>{c.label}</b><small className="muted">{c.blurb}</small></span>
                  </button>
                ))}
              </div>
              <p className="group-title">A metric you import</p>
              <input className="search" type="search" placeholder="Search" value={q} onChange={(e) => setQ(e.target.value)} aria-label="Search metrics" />
              <div className="group">
                {metrics.filter((m) => !q || m.name.toLowerCase().includes(q.toLowerCase())).slice(0, 40).map((m) => (
                  <button key={m.metric} type="button" className="row" onClick={() => add(blank("bar", m.metric, m.name))}>
                    <span className="row-label">{m.name}</span>
                  </button>
                ))}
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export async function summaryHasMetric(): Promise<{ id: number; widgets: Widget[] } | null> {
  const r = await api<{ dashboards: Dashboard[] }>("/dashboards");
  const summary = r.dashboards.find((b) => b.builtin === "summary");
  if (!summary) return null;
  return { id: summary.id, widgets: summary.widgets };
}

export async function toggleSummaryMetric(metric: string, name: string) {
  const summary = await summaryHasMetric();
  if (!summary) return false;
  const has = summary.widgets.some((w) => w.metric === metric);
  const widgets = has
    ? summary.widgets.filter((w) => w.metric !== metric)
    : [...summary.widgets, blank("bar", metric, name)];
  await api(`/dashboards/${summary.id}`, { method: "PUT", body: { widgets } });
  return !has;
}

export function metricOnBoard(widgets: Widget[] | undefined, metric: string) {
  return !!widgets?.some((w) => w.metric === metric);
}

export type { BrowseMetric };
