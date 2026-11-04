// Awards: computed badges grouped by section, detail sheet, Apple-only checklist.
import { useState } from "react";
import { api, explain } from "../api";
import { Link } from "../router";
import { useApi } from "./meta";
import { Badge } from "./rings";
import { LargeTitle, NavBar, Sheet } from "./ui";

export type AwardOut = {
  key: string; name: string; group: string; description: string; color: string; shape: string; mark?: string; detail?: string | null;
  earned: { first: string; latest: string; count: number; dates: string[] } | null;
  progress: [number, number] | null;
};
type AppleOnly = { key: string; name: string; description: string; earned: boolean; earned_date: string | null };
type Resp = { enabled: boolean; awards: AwardOut[]; apple_only: AppleOnly[] };

const fmtD = (d: string) => new Date(d + "T12:00:00").toLocaleDateString([], { month: "short", day: "numeric", year: "numeric" });
const SECTIONS = ["Close Your Rings", "Workouts", "Streaks & Perfect Weeks", "Personal Records"];

function Cell({ a, onOpen }: { a: AwardOut; onOpen: () => void }) {
  return (
    <button className="awardcell" onClick={onOpen}>
      <Badge color={a.color} shape={a.shape} mark={a.mark} size={92} dim={!a.earned} />
      <b>{a.name}</b>
      {a.earned ? (
        <small className="muted">{fmtD(a.earned.latest)}{a.earned.count > 1 ? ` · ×${a.earned.count}` : ""}</small>
      ) : a.progress ? (
        <small className="muted num">{Math.min(a.progress[0], a.progress[1]).toLocaleString()}/{a.progress[1].toLocaleString()}</small>
      ) : <small className="muted">Not yet earned</small>}
    </button>
  );
}

export default function Awards() {
  const r = useApi<Resp>("/awards");
  const [open, setOpen] = useState<AwardOut | null>(null);
  const [apple, setApple] = useState<AppleOnly[] | null>(null);
  const [err, setErr] = useState("");
  const d = r.data;
  const ao = apple ?? d?.apple_only ?? [];
  if (d && !d.enabled)
    return (
      <div className="page">
        <NavBar to="/" />
        <LargeTitle title="Awards" avatar={false} />
        <div className="card"><p>Awards are off. Turn them on in <Link to="/account/awards">Account → Awards</Link>. They work best after importing your full export.zip.</p></div>
      </div>
    );
  const recent = (d?.awards ?? []).filter((a) => a.earned).sort((a, b) => b.earned!.latest.localeCompare(a.earned!.latest)).slice(0, 6);
  const setAO = async (k: string, earned: boolean, date?: string) => {
    setErr("");
    try {
      await api(`/awards/apple/${k}`, { method: "PUT", body: { earned, date: date || null } });
      setApple(ao.map((x) => (x.key === k ? { ...x, earned, earned_date: earned ? date || null : null } : x)));
    } catch (e) { setErr(explain(e)); }
  };
  return (
    <div className="page awards">
      <NavBar to="/" />
      <LargeTitle title="Awards" avatar={false} />
      {recent.length > 0 && (
        <section><h2>Recent</h2><div className="awardgrid">{recent.map((a) => <Cell key={a.key} a={a} onOpen={() => setOpen(a)} />)}</div></section>
      )}
      {SECTIONS.map((s) => {
        const list = (d?.awards ?? []).filter((a) => a.group === s);
        if (!list.length) return null;
        return (
          <section key={s}>
            <div className="sec-head"><h2>{s === "Streaks & Perfect Weeks" ? "Streaks" : s}</h2><span className="muted small">{list.filter((a) => a.earned).length} of {list.length}</span></div>
            <div className="awardgrid">{list.map((a) => <Cell key={a.key} a={a} onOpen={() => setOpen(a)} />)}</div>
          </section>
        );
      })}
      <section>
        <div className="sec-head"><h2>Apple-only awards</h2></div>
        <p className="muted small">Tracked in Apple Fitness only. Health data can't prove these, so mark the ones you've earned.</p>
        {err && <p className="error">{err}</p>}
        <div className="group">
          {ao.map((x) => (
            <div key={x.key} className="row static appleonly">
              <label className="row-label">
                <input type="checkbox" checked={x.earned} onChange={(e) => setAO(x.key, e.target.checked, x.earned_date ?? undefined)} />
                <span><b>{x.name}</b><small className="muted">{x.description}</small></span>
              </label>
              {x.earned && <input type="date" value={x.earned_date ?? ""} aria-label={`${x.name} date`} onChange={(e) => setAO(x.key, true, e.target.value)} />}
            </div>
          ))}
        </div>
      </section>
      {open && (
        <Sheet title="" onClose={() => setOpen(null)}>
          <div className="awarddetail">
            <Badge color={open.color} shape={open.shape} mark={open.mark} size={180} dim={!open.earned} />
            <h2>{open.name}</h2>
            <p className="muted">{open.description}</p>
            {open.detail && <p className="big num">{open.detail}</p>}
            {open.earned ? (
              <>
                <p>Earned {open.earned.count > 1 ? `${open.earned.count} times` : "once"} · first {fmtD(open.earned.first)}{open.earned.count > 1 ? `, latest ${fmtD(open.earned.latest)}` : ""}</p>
                {open.earned.dates.length > 1 && <div className="chips">{open.earned.dates.slice(0, 20).map((x) => <span key={x} className="chip">{fmtD(x)}</span>)}</div>}
              </>
            ) : open.progress ? (
              <div className="progress"><i style={{ width: `${Math.min(100, (open.progress[0] / open.progress[1]) * 100)}%` }} /><span className="num">{open.progress[0].toLocaleString()} / {open.progress[1].toLocaleString()}</span></div>
            ) : <p className="muted">Not earned yet.</p>}
          </div>
        </Sheet>
      )}
    </div>
  );
}
