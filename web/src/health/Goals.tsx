// Manual Activity ring goals editor and the "no goals today" alert.
import { useEffect, useState } from "react";
import { api, explain } from "../api";
import { Link, navigate } from "../router";
import { useHealth, Units } from "./ctx";
import { today } from "./range";
import { Group, NavBar } from "./ui";

export type ManualGoal = {
  id: number;
  effective_from: string;
  move_kcal: number | null;
  exercise_min: number | null;
  stand_hours: number | null;
  created_at: string;
};

const KJ = 4.184;
const SESSION_KEY = "hv_goals_alert_dismissed";

/** Session-dismissible banner when today has no resolved ring goals. */
export function NoGoalsAlert({ show }: { show: boolean }) {
  const [dismissed, setDismissed] = useState(() => {
    try { return sessionStorage.getItem(SESSION_KEY) === "1"; } catch { return false; }
  });
  if (!show || dismissed) return null;
  const dismiss = () => {
    try { sessionStorage.setItem(SESSION_KEY, "1"); } catch { /* ignore */ }
    setDismissed(true);
  };
  return (
    <aside className="card goals-alert" role="status">
      <div className="goals-alert-body">
        <b>Set your Activity ring goals</b>
        <p className="muted small">
          We do not have Move, Exercise or Stand goals for today. Goals only come from an Apple Health export
          or from ones you set here. Tell us what your goals are and when they started.
        </p>
        <div className="actions">
          <button type="button" className="pill green" onClick={() => navigate("/account/goals")}>Set goals</button>
          <button type="button" className="pill" onClick={dismiss}>Not now</button>
        </div>
      </div>
    </aside>
  );
}

function Sub({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="page sub">
      <NavBar to="/account" title={title} />
      {children}
    </div>
  );
}

/** Display a stored kcal value in the user's energy unit. */
function showMove(kcal: number | null, u: Units): string {
  if (kcal == null) return "—";
  const v = u.energy === "kj" ? kcal * KJ : kcal;
  return `${Math.round(v).toLocaleString()} ${u.energy === "kj" ? "kJ" : "kcal"}`;
}

/** Parse a user-typed move value back to kcal for storage. */
function parseMove(raw: string, u: Units): number | null {
  const t = raw.trim();
  if (t === "") return null;
  const n = Number(t);
  if (!Number.isFinite(n) || n < 0) return null;
  return u.energy === "kj" ? n / KJ : n;
}

const shortDay = (d: string) => new Date(d + "T12:00:00").toLocaleDateString([], { month: "short", day: "numeric", year: "numeric" });

export function GoalsPage() {
  const { units } = useHealth();
  const [goals, setGoals] = useState<ManualGoal[]>([]);
  const [msg, setMsg] = useState("");
  const [editId, setEditId] = useState<number | null>(null);
  const [from, setFrom] = useState(today());
  const [move, setMove] = useState("");
  const [exercise, setExercise] = useState("");
  const [stand, setStand] = useState("");
  const energyLabel = units.energy === "kj" ? "kJ" : "kcal";

  const load = async () => {
    try {
      const r = await api<{ goals: ManualGoal[] }>("/me/manual-goals");
      setGoals(r.goals);
    } catch (e) { setMsg(explain(e)); }
  };
  useEffect(() => { void load(); }, []);

  const resetForm = (g?: ManualGoal | null) => {
    if (g) {
      setEditId(g.id);
      setFrom(g.effective_from);
      setMove(g.move_kcal == null ? "" : String(Math.round(units.energy === "kj" ? g.move_kcal * KJ : g.move_kcal)));
      setExercise(g.exercise_min == null ? "" : String(Math.round(g.exercise_min)));
      setStand(g.stand_hours == null ? "" : String(Math.round(g.stand_hours)));
    } else {
      setEditId(null);
      setFrom(today());
      setMove("");
      setExercise("");
      setStand("");
    }
  };

  const save = async () => {
    setMsg("");
    const body = {
      effective_from: from,
      move_kcal: parseMove(move, units),
      exercise_min: exercise.trim() === "" ? null : Number(exercise),
      stand_hours: stand.trim() === "" ? null : Number(stand),
    };
    try {
      if (editId != null) await api(`/me/manual-goals/${editId}`, { method: "PUT", body });
      else await api("/me/manual-goals", { method: "POST", body });
      setMsg("Saved.");
      resetForm(null);
      await load();
    } catch (e) { setMsg(explain(e)); }
  };

  const remove = async (id: number) => {
    setMsg("");
    try {
      await api(`/me/manual-goals/${id}`, { method: "DELETE" });
      if (editId === id) resetForm(null);
      await load();
    } catch (e) { setMsg(explain(e)); }
  };

  return (
    <Sub title="Activity Ring Goals">
      <section className="card">
        <p className="muted small">
          Goals from an Apple Health export always take priority over goals you set here.
          A goal you set starts on the date below and rolls forward day by day until a later
          goal you set, or an export goal, takes over. Leave a ring blank to leave it unset.
          There are no default goals.
        </p>
      </section>
      <section className="card form">
        <h3>{editId != null ? "Edit entry" : "Set goals"}</h3>
        <label className="field">Start date<input type="date" value={from} onChange={(e) => setFrom(e.target.value)} /></label>
        <label className="field">Move ({energyLabel})<input type="number" min={0} step="1" value={move} onChange={(e) => setMove(e.target.value)} placeholder="optional" /></label>
        <label className="field">Exercise (min)<input type="number" min={0} step="1" value={exercise} onChange={(e) => setExercise(e.target.value)} placeholder="optional" /></label>
        <label className="field">Stand (hrs)<input type="number" min={0} max={24} step="1" value={stand} onChange={(e) => setStand(e.target.value)} placeholder="optional" /></label>
        <div className="actions">
          <button type="button" className="pill green" onClick={save}>{editId != null ? "Update" : "Save"}</button>
          {editId != null && <button type="button" className="pill" onClick={() => resetForm(null)}>Cancel</button>}
          {msg && <span className="muted">{msg}</span>}
        </div>
      </section>
      <Group title="Past entries" foot="Newest first. Edit or delete any entry.">
        {goals.length === 0 && <p className="muted small" style={{ padding: "8px 16px" }}>No goals set yet.</p>}
        {goals.map((g) => (
          <div key={g.id} className="row goals-hist">
            <div className="row-label" style={{ flex: 1 }}>
              <b>{shortDay(g.effective_from)}</b>
              <small className="muted"> · Move {showMove(g.move_kcal, units)} · Exercise {g.exercise_min == null ? "—" : `${Math.round(g.exercise_min)} min`} · Stand {g.stand_hours == null ? "—" : `${Math.round(g.stand_hours)} hrs`}</small>
            </div>
            <button type="button" className="linkish" onClick={() => resetForm(g)}>Edit</button>
            <button type="button" className="linkish" onClick={() => void remove(g.id)}>Delete</button>
          </div>
        ))}
      </Group>
      <p className="muted small" style={{ padding: "0 4px" }}>
        Prefer an export? <Link to="/settings/import">Import Health Data</Link> brings in ActivitySummary goals from export.xml.
      </p>
    </Sub>
  );
}
