// Awards: Apple Fitness awards the user marks as earned. Apple's award list is
// not in the Health export and healthvibe computes no awards or streaks.
import { useState } from "react";
import { api, explain } from "../api";
import { useApi } from "./meta";
import { LargeTitle, NavBar } from "./ui";

export type AppleAward = { key: string; name: string; description: string; earned: boolean; earned_date: string | null };
export type AwardsResp = { apple_only: AppleAward[] };

export default function Awards() {
  const r = useApi<AwardsResp>("/awards");
  const [apple, setApple] = useState<AppleAward[] | null>(null);
  const [err, setErr] = useState("");
  const ao = apple ?? r.data?.apple_only ?? [];
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
      <section>
        <p className="muted small">Apple Fitness awards aren't included in the Health export, and healthvibe doesn't work them out from your data. Mark the ones you've earned in Apple Fitness.</p>
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
    </div>
  );
}
