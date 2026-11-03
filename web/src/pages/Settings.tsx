import { useState } from "react";
import { api, User } from "../api";
import { applyTheme, DARK, LIGHT, Mode, ThemePrefs } from "../theme";
import { ChangePassword } from "./Auth";
import { SyncSettings } from "../components/Sync";

export default function Settings({ user, onUser }: { user: User; onUser: (u: User) => void }) {
  const [t, setT] = useState<ThemePrefs>({ mode: user.theme_mode, light: user.theme_light, dark: user.theme_dark });
  const [saved, setSaved] = useState<string | null>(null);
  const save = async (next: ThemePrefs) => {
    setT(next);
    applyTheme(next);
    try {
      const r = await api("/me/theme", { method: "PUT", body: next });
      onUser(r.user);
      setSaved("Saved");
    } catch {
      setSaved("Couldn't save");
    }
  };
  return (
    <div>
      <header className="pagehead"><h1>Settings</h1></header>
      <section className="card">
        <div className="cardhead">
          <h2>Appearance</h2>
          {saved && <span className="muted">{saved}</span>}
        </div>
        <div className="segmented" role="radiogroup" aria-label="Mode">
          {(["light", "dark", "system"] as Mode[]).map((m) => (
            <button key={m} role="radio" aria-checked={t.mode === m} className={t.mode === m ? "on" : ""} onClick={() => save({ ...t, mode: m })}>
              {m === "system" ? "Follow system" : m[0].toUpperCase() + m.slice(1)}
            </button>
          ))}
        </div>
        <h3>Light palette</h3>
        <Palettes set={LIGHT} value={t.light} onPick={(light) => save({ ...t, light })} />
        <h3>Dark palette</h3>
        <Palettes set={DARK} value={t.dark} onPick={(dark) => save({ ...t, dark })} />
        <p className="muted foot">Follow system uses the light or dark palette to match your device.</p>
      </section>
      <SyncSettings />
      <ChangePassword user={user} onDone={onUser} />
    </div>
  );
}

function Palettes(props: { set: typeof LIGHT; value: string; onPick: (k: string) => void }) {
  return (
    <div className="palettes">
      {Object.entries(props.set).map(([k, p]) => (
        <button key={k} className={`palette ${props.value === k ? "on" : ""}`} onClick={() => props.onPick(k)}
          style={{ background: p.vars["--bg"], color: p.vars["--text"], borderColor: props.value === k ? p.vars["--accent"] : p.vars["--border"] }}>
          <span className="swatches">
            {["--move", "--exercise", "--stand", "--accent", "--deep"].map((v) => (
              <i key={v} style={{ background: p.vars[v] }} />
            ))}
          </span>
          <span className="pcard" style={{ background: p.vars["--surface"], color: p.vars["--muted"] }}>Aa</span>
          {p.name}
        </button>
      ))}
    </div>
  );
}
