import { useEffect, useState } from "react";
import { api, explain, HaeInfo, HaeToken } from "../api";
import { Link } from "../router";

export const SPONSORS_URL = "https://github.com/sponsors/mortbrew";

export const useHae = (pollMs?: number) => {
  const [info, setInfo] = useState<HaeInfo | null>(null);
  const reload = () => api<HaeInfo>("/sync/hae").then(setInfo).catch(() => {});
  useEffect(() => {
    reload();
    if (!pollMs) return;
    const t = setInterval(reload, pollMs);
    return () => clearInterval(t);
  }, [pollMs]);
  return { info, reload };
};

function Copy({ label, value }: { label: string; value: string }) {
  const [done, setDone] = useState(false);
  return (
    <div className="copyrow">
      <span className="muted small">{label}</span>
      <code className="copyval">{value}</code>
      <button
        className="small"
        onClick={() => navigator.clipboard?.writeText(value).then(() => { setDone(true); setTimeout(() => setDone(false), 1500); })}
      >
        {done ? "Copied" : "Copy"}
      </button>
    </div>
  );
}

/** Create the upload token and show exactly what to paste into Health Auto Export. */
export function HaeConnect({ onToken }: { onToken?: () => void }) {
  const { info, reload } = useHae();
  const [tok, setTok] = useState<HaeToken | null>(null);
  const [err, setErr] = useState<string | null>(null);
  if (!info) return null;
  const create = async () => {
    if (info.has_token && !confirm("Make a new token? The old one stops working right away, so update the header in Health Auto Export too.")) return;
    setErr(null);
    try {
      setTok(await api<HaeToken>("/sync/hae/token", { method: "POST" }));
      reload();
      onToken?.();
    } catch (e) {
      setErr(explain(e));
    }
  };
  const url = tok?.url ?? info.url;
  const pubSet = tok?.public_url_set ?? info.public_url_set;
  return (
    <div className="haeconnect">
      <Copy label="URL" value={url} />
      {!pubSet && (
        <p className="error small">
          HEALTHVIBE_PUBLIC_URL is not set, so this is the address your browser used. Set it to the public HTTPS
          address your iPhone can reach.
        </p>
      )}
      {tok ? (
        <>
          <Copy label="Header key" value="Authorization" />
          <Copy label="Header value" value={`Bearer ${tok.token}`} />
          <p className="muted small">
            Copy the header value now. It is shown only once. If Health Auto Export won't take an Authorization
            header, use the key <code>{tok.header}</code> with the value <code>{tok.token}</code> instead.
          </p>
        </>
      ) : (
        <p className="muted small">
          {info.has_token
            ? "You already have a token. It isn't shown again; make a new one if you lost it."
            : "Make your personal upload token. It goes into Health Auto Export as a header."}
        </p>
      )}
      {err && <p className="error">{err}</p>}
      <button onClick={create} className={info.has_token && !tok ? "small" : ""}>
        {info.has_token ? "Make a new token" : "Create my token"}
      </button>
    </div>
  );
}

/** Live check for the first upload. */
export function FirstData() {
  const { info } = useHae(4000);
  if (!info) return null;
  const last = info.device?.last_upload_at;
  if (!info.has_token) return <p className="muted">Create your token first.</p>;
  if (!last)
    return (
      <div className="card notice live">
        <span className="pulse" aria-hidden /> Waiting for the first data from Health Auto Export… (checks every few seconds)
      </div>
    );
  return (
    <div className="card notice ok">
      ✓ Received <b>{info.received.toLocaleString()}</b> readings so far · last upload {new Date(last).toLocaleString()}
    </div>
  );
}

const METRICS = [
  "Step Count", "Walking + Running Distance", "Active Energy", "Resting Energy", "Apple Exercise Time",
  "Apple Stand Hour", "Apple Stand Time", "Flights Climbed", "Resting Heart Rate",
  "Walking Heart Rate Average", "Heart Rate Variability", "VO2 Max", "Sleep Analysis", "Respiratory Rate",
  "Blood Oxygen Saturation", "Apple Sleeping Wrist Temperature", "Weight & Body Mass", "Body Fat Percentage",
];

function MetricChecklist() {
  const [on, setOn] = useState<Set<string>>(new Set());
  return (
    <ul className="checklist">
      {METRICS.map((m) => (
        <li key={m}>
          <label>
            <input type="checkbox" checked={on.has(m)} onChange={() => setOn((s) => { const n = new Set(s); n.has(m) ? n.delete(m) : n.add(m); return n; })} />{" "}
            {m}
          </label>
        </li>
      ))}
    </ul>
  );
}

/** The Health Auto Export steps, shared by onboarding and Help. `connect` is shown at the URL/header step. */
export function HaeSteps({ connect, start = 1 }: { connect: React.ReactNode; start?: number }) {
  let n = start;
  const S = (p: { title: string; children: React.ReactNode }) => (
    <li className="step">
      <div className="stepnum">{n++}</div>
      <div className="stepbody">
        <h3>{p.title}</h3>
        {p.children}
      </div>
    </li>
  );
  return (
    <ol className="guide">
      <S title="Install Health Auto Export">
        <p>
          Get <b>Health Auto Export – JSON+CSV</b> from the App Store and open it once. Tap <b>Allow</b> for the Health
          types you want healthvibe to show. Sending to a server (REST API automations) needs the app's Premium tier.
        </p>
        <a className="button small" href="https://apps.apple.com/app/id1115567069">Open in the App Store</a>
      </S>
      <S title="Create a REST API automation">
        <p>In the app: <b>Automations</b> → <b>+</b> (New Automation) → <b>REST API</b>. Name it <b>healthvibe</b>.</p>
      </S>
      <S title="Paste the URL and header">
        <p>Paste the URL. Under <b>Headers</b>, add one header with the key and value below.</p>
        {connect}
      </S>
      <S title="Choose JSON and what to send">
        <ul>
          <li><b>Export Format</b>: JSON.</li>
          <li><b>Data Type</b>: Health Metrics. <b>Turn on all metrics except Heart Rate</b> (Select All, then untick
            Heart Rate; its per-minute samples are huge and slow every sync). Keep Resting Heart Rate, Walking Heart Rate
            Average and Heart Rate Variability on. healthvibe stores everything it receives, so you don't choose anything
            in healthvibe. At minimum, make sure these are on:
            <MetricChecklist />
          </li>
          <li><b>Aggregate Data</b>: off, so every reading and every sleep stage arrives. (On also works; sleep is then a nightly summary.)</li>
          <li><b>Batch Requests</b>: on, so a long history is sent in pieces.</li>
          <li><b>Date Range</b>: since the last sync if offered, otherwise the last few days. Re-sent readings are recognised and never doubled.</li>
        </ul>
        <p className="muted small">Want workouts too? Make a second automation the same way with <b>Data Type: Workouts</b> (version 2).</p>
      </S>
      <S title="Choose how often it syncs">
        <p>
          Set <b>Sync Cadence</b> (for example every hour) and turn the automation on. iOS only lets apps read Health
          data <b>while your iPhone is unlocked</b>, so syncs happen when you use your phone. Keep Background App Refresh
          on for Health Auto Export.
        </p>
      </S>
      <S title="Test it">
        <p>Tap <b>Manual Export</b> (Export now) in the automation, pick the last few days, and send. This updates live:</p>
        <FirstData />
      </S>
    </ol>
  );
}

export function SponsorNote() {
  return (
    <p className="muted small sponsor">
      Why a paid app? A free healthvibe iPhone app needs a paid Apple developer account. If{" "}
      <a href={SPONSORS_URL}>GitHub Sponsors</a> and donations cover the yearly Apple developer fee, I'll build a free
      sync app for everyone. — Toby
    </p>
  );
}

/** Settings card. */
export function HaeSettings() {
  const { info } = useHae();
  return (
    <section className="card" id="sync">
      <div className="cardhead"><h2>iPhone sync (Health Auto Export)</h2></div>
      {info?.device?.last_upload_at ? (
        <p className="muted small">
          Last upload {new Date(info.device.last_upload_at).toLocaleString()} · {info.received.toLocaleString()} readings received with this token
        </p>
      ) : (
        <p className="muted small">No uploads yet.</p>
      )}
      <HaeConnect />
      <div className="row">
        <Link to="/help/health-auto-export" className="button small">Setup guide</Link>
        <Link to="/welcome" className="button small">Open the welcome setup again</Link>
      </div>
    </section>
  );
}
