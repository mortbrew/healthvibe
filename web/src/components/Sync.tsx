import { useEffect, useState } from "react";
import QRCode from "qrcode";
import { api, explain, SetupCode, SyncInfo } from "../api";
import { Link } from "../router";

export const useSync = () => {
  const [info, setInfo] = useState<SyncInfo | null>(null);
  const reload = () => api<SyncInfo>("/sync").then(setInfo);
  useEffect(() => {
    reload();
  }, []);
  return { info, reload, setInfo };
};

const TRIGGER_LABEL: Record<string, string> = {
  alarm_stopped: "When I stop my alarm",
  time_of_day: "At a set time",
  arrive_home: "When I arrive home",
  charger: "When I plug in the charger",
  manual: "Manual only",
};

/** Settings card: choose what the Shortcut sends. */
export function SyncSettings() {
  const { info, setInfo } = useSync();
  const [sel, setSel] = useState<Set<string>>(new Set());
  const [msg, setMsg] = useState<string | null>(null);
  useEffect(() => {
    if (info) setSel(new Set(info.enabled));
  }, [info?.enabled.join()]);
  if (!info) return null;
  const areas = [...new Set(info.metrics.map((m) => m.area))];
  const toggle = (k: string) => {
    const n = new Set(sel);
    n.has(k) ? n.delete(k) : n.add(k);
    setSel(n);
  };
  const save = async (patch: Partial<{ trigger: string; notify: string }> = {}) => {
    setMsg(null);
    try {
      const r = await api("/sync", { method: "PUT", body: { enabled: [...sel], trigger: info.trigger, notify: info.notify, ...patch } });
      setInfo({ ...info, ...r });
      setMsg("Saved");
    } catch (e) {
      setMsg(explain(e));
    }
  };
  const preset = (keys: string[]) => setSel(new Set(keys));
  return (
    <section className="card" id="sync">
      <div className="cardhead">
        <h2>iPhone Shortcut sync</h2>
        {msg && <span className="muted">{msg}</span>}
      </div>
      <p className="muted">
        Pick what the Shortcut sends. Only these are read on your phone, and the server drops anything else.
        {!info.configured && " Save once to get your setup QR code."}
      </p>
      <div className="row">
        <button className="small" onClick={() => preset(info.metrics.filter((m) => m.recommended).map((m) => m.key))}>Recommended</button>
        <button className="small" onClick={() => preset(info.metrics.map((m) => m.key))}>Everything</button>
        <button className="small" onClick={() => preset([])}>None</button>
      </div>
      <div className="metricgrid">
        {areas.map((a) => (
          <fieldset key={a}>
            <legend>{a}</legend>
            {info.metrics.filter((m) => m.area === a).map((m) => (
              <label key={m.key} className="check">
                <input type="checkbox" checked={sel.has(m.key)} onChange={() => toggle(m.key)} />
                {m.name}
              </label>
            ))}
          </fieldset>
        ))}
      </div>
      <label className="field">
        When it runs
        <select value={info.trigger} onChange={(e) => save({ trigger: e.target.value })}>
          {info.triggers.map((t) => <option key={t} value={t}>{TRIGGER_LABEL[t] ?? t}</option>)}
        </select>
      </label>
      <label className="check">
        <input type="checkbox" checked={info.notify === "always"} onChange={(e) => save({ notify: e.target.checked ? "always" : "errors_only" })} />
        Show a notification after every run (otherwise only when something fails)
      </label>
      <div className="row">
        <button onClick={() => save()} disabled={sel.size === 0}>Save ({sel.size} selected)</button>
        {info.configured && <Link to="/help/shortcut" className="button small">Get setup QR code</Link>}
      </div>
      <DeviceLine info={info} />
    </section>
  );
}

export function DeviceLine({ info }: { info: SyncInfo }) {
  const d = info.device;
  if (!d) return <p className="muted small">No iPhone connected yet.</p>;
  const when = (t: string | null) => (t ? new Date(t).toLocaleString() : "never");
  return (
    <p className="muted small">
      Connected: <b>{d.device ?? "iPhone"}</b> (Shortcut {d.shortcut_version ?? "?"}) · last upload {when(d.last_upload_at)}
    </p>
  );
}

/** QR code 2: a fresh one-time setup code, regenerated on every page load. */
export function SetupQr() {
  const { info } = useSync();
  const [code, setCode] = useState<SetupCode | null>(null);
  const [img, setImg] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const fresh = async () => {
    setErr(null);
    try {
      const c = await api<SetupCode>("/sync/setup-code", { method: "POST" });
      setCode(c);
      setImg(await QRCode.toDataURL(c.link, { margin: 1, width: 220 }));
    } catch (e) {
      setErr(explain(e));
    }
  };
  useEffect(() => {
    if (info?.configured) fresh();
  }, [info?.configured]);
  if (!info) return null;
  if (!info.configured)
    return (
      <div className="card notice">
        First <Link to="/settings">choose what to sync in Settings</Link>. Your setup QR code appears here afterwards.
      </div>
    );
  return (
    <div className="qrbox">
      {img ? <img src={img} alt="Setup QR code" className="qr" /> : <div className="qr placeholder">…</div>}
      <div>
        {err && <p className="error">{err}</p>}
        {code && (
          <>
            <p className="small">Server: <code>{code.server}</code></p>
            {!code.public_url_set && (
              <p className="error small">
                HEALTHVIBE_PUBLIC_URL is not set, so this is the address your browser used. Set it to the public
                HTTPS address your iPhone can reach.
              </p>
            )}
            <p className="small">Setup code: <code>{code.code}</code></p>
            <p className="muted small">Works once, until {new Date(code.expires_at).toLocaleTimeString()}. Reload the page for a new one.</p>
            <button className="small" onClick={fresh}>New code</button>
          </>
        )}
      </div>
    </div>
  );
}
