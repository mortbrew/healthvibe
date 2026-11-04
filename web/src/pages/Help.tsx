import { useState } from "react";
import { api, explain } from "../api";
import { SetupQr, useSync, DeviceLine } from "../components/Sync";
import { Link } from "../router";
import { HaeConnect, HaeSteps, SponsorNote } from "../components/Hae";

/** Placeholder for a screenshot that hasn't been captured on a real iPhone yet. */
function Shot({ label }: { label: string }) {
  return (
    <figure className="shot" aria-label={`Screenshot placeholder: ${label}`}>
      <div className="phone">
        <div className="notch" />
        <span>{label}</span>
      </div>
      <figcaption className="muted">Screenshot coming soon</figcaption>
    </figure>
  );
}

function Step(props: { n: number; title: string; shot?: string; children: React.ReactNode }) {
  return (
    <li className="step">
      <div className="stepnum">{props.n}</div>
      <div className="stepbody">
        <h3>{props.title}</h3>
        {props.children}
      </div>
      {props.shot && <Shot label={props.shot} />}
    </li>
  );
}

export function HelpIndex() {
  return (
    <div>
      <header className="pagehead"><h1>Help</h1></header>
      <div className="grid">
        <Link to="/help/health-auto-export" className="card linkcard">
          <h2>Sync your iPhone with Health Auto Export</h2>
          <p className="muted">Send Apple Health data to healthvibe automatically, step by step.</p>
        </Link>
        <Link to="/welcome" className="card linkcard">
          <h2>Open the welcome setup again</h2>
          <p className="muted">The guided first-run setup, with a live check for your first upload.</p>
        </Link>
        <Link to="/import" className="card linkcard">
          <h2>Import an export.zip</h2>
          <p className="muted">One-off import of your full history from the Health app.</p>
        </Link>
      </div>
      <SponsorNote />
    </div>
  );
}

export function HaeGuide() {
  return (
    <div className="help">
      <header className="pagehead">
        <p className="eyebrow"><Link to="/help">Help</Link></p>
        <h1>Sync your iPhone with Health Auto Export</h1>
      </header>
      <p className="lead">
        Health Auto Export reads Apple Health on your iPhone and posts it to healthvibe as JSON (REST API automation).
        Data only syncs while your iPhone is unlocked, because iOS locks Health data otherwise.
      </p>
      <HaeSteps connect={<HaeConnect />} />
      <SponsorNote />
    </div>
  );
}

// The DIY Shortcut guides below are kept but no longer linked from Help (v1 uses Health Auto Export).
const TRIGGERS = {
  alarm: {
    label: "When I stop my alarm (recommended)",
    steps: ["Choose Alarm → Is Stopped → Wake-Up (or Any)."],
    note: "Your phone is usually unlocked right then, so Health can be read.",
  },
  time: {
    label: "At a set time",
    steps: ["Choose Time of Day, pick a time, and set it to Daily."],
    note: "If the phone is locked at that time the run is skipped; the next run catches up.",
  },
  home: {
    label: "When I arrive home",
    steps: ["Choose Arrive → pick your Home location."],
    note: "Needs location permission. Locked-phone runs catch up next time.",
  },
  charger: {
    label: "When I plug in the charger",
    steps: ["Choose Charger → Is Connected."],
    note: "Often fires while locked at night, so pair it with another trigger.",
  },
  manual: {
    label: "Manual only",
    steps: [],
    note: "Run it from the widget, the Action button, or by saying \"healthvibe\" to Siri.",
  },
} as const;

export function ShortcutGuide() {
  const [trigger, setTrigger] = useState<keyof typeof TRIGGERS>("alarm");
  const t = TRIGGERS[trigger];
  return (
    <div className="help">
      <header className="pagehead">
        <p className="eyebrow"><Link to="/help">Help</Link></p>
        <h1>Set up the iPhone Shortcut</h1>
      </header>
      <div className="card notice">
        <b>No ready-made download yet.</b> The signed Shortcut file (QR code 1) isn't published, so for now
        {" "}<Link to="/help/build">build it yourself</Link> (about 20 minutes), then come back for step 3.
      </div>
      <ol className="guide">
        <Step n={1} title="Choose what to sync">
          <p>In <Link to="/settings">Settings → iPhone Shortcut sync</Link>, tick what you want to send and save. Only those types are read on the phone.</p>
        </Step>
        <Step n={2} title="Get the Shortcut on your iPhone">
          <p>Follow <Link to="/help/build">Build the Shortcut yourself</Link>. Run it once from the Shortcuts app and tap <b>Allow</b> for each Health type it asks about (you can review this later in Settings → Health → Data Access → Shortcuts).</p>
        </Step>
        <Step n={3} title="Connect it to your account">
          <p>On the first run the Shortcut opens the camera. Scan this code. It is new every time this page loads, works once, and expires after 10 minutes. The Shortcut saves your server address and personal upload token to <code>hv-config.json</code> in iCloud Drive → Shortcuts → healthvibe.</p>
          <SetupQr />
        </Step>
        <Step n={4} title="Choose when it runs">
          <div className="segmented vertical" role="radiogroup">
            {Object.entries(TRIGGERS).map(([k, v]) => (
              <button key={k} role="radio" aria-checked={trigger === k} className={trigger === k ? "on" : ""} onClick={() => setTrigger(k as keyof typeof TRIGGERS)}>
                {v.label}
              </button>
            ))}
          </div>
          <p className="muted">{t.note}</p>
        </Step>
        {trigger !== "manual" && (
          <Step n={5} title="Create the automation" shot="New Automation">
            <ol className="steps">
              <li>Open <b>Shortcuts</b> → <b>Automation</b> tab → tap <b>+</b>.
                {" "}<a href="shortcuts://automations" className="button small">Open Shortcuts automations</a>
              </li>
              {t.steps.map((s) => <li key={s}>{s}</li>)}
              <li>Choose <b>Run Immediately</b> (on iOS 27: turn <b>Confirm Before Run</b> off). Turn off <b>Notify When Run</b> if you like.</li>
              <li>Tap <b>Next</b> → pick the <b>healthvibe</b> shortcut → <b>Done</b>.</li>
            </ol>
          </Step>
        )}
        <Step n={trigger === "manual" ? 5 : 6} title="Check it worked">
          <p>After the next run, open the <Link to="/">Summary</Link>; your latest steps and sleep should appear. Each run sends everything since the last successful upload, so missed days catch up on their own.</p>
        </Step>
      </ol>
      <p className="muted">Something not right? <Link to="/help/messed-up">Reset and reinstall the Shortcut</Link>.</p>
    </div>
  );
}

export function MessedUp() {
  return (
    <div className="help">
      <header className="pagehead">
        <p className="eyebrow"><Link to="/help">Help</Link></p>
        <h1>Help, I messed up my Shortcut</h1>
      </header>
      <p className="lead">No problem. These steps remove everything healthvibe put on your phone and set it up fresh. Your data on the server is kept.</p>
      <ol className="guide">
        <Step n={1} title="Delete the automation" shot="Swipe to delete automation">
          <p>Shortcuts → <b>Automation</b> → swipe left on the healthvibe automation → <b>Delete</b>.</p>
        </Step>
        <Step n={2} title="Delete the Shortcut" shot="Long-press → Delete">
          <p>Shortcuts → long-press <b>healthvibe</b> → <b>Delete</b>.</p>
        </Step>
        <Step n={3} title="Delete the saved settings file" shot="Files → iCloud Drive">
          <p>Files → iCloud Drive → Shortcuts → <code>healthvibe</code> → delete <code>hv-config.json</code> (or the whole folder).</p>
        </Step>
        <Step n={4} title="Reset your upload token">
          <p>This immediately stops the old Shortcut from uploading. Your sync settings and stored data are kept.</p>
          <ResetToken />
        </Step>
        <Step n={5} title="Reinstall">
          <p>Build it again with <Link to="/help/build">Build the Shortcut yourself</Link> (a signed download, QR code 1, isn't published yet).</p>
        </Step>
        <Step n={6} title="Reconfigure">
          <p>Run it once and scan this fresh setup code, then create the automation again with the <Link to="/help/shortcut">setup guide</Link>.</p>
          <SetupQr />
        </Step>
      </ol>
      <p className="muted">Optional: Settings → Health → Data Access → Shortcuts to review or reset Health permissions.</p>
    </div>
  );
}

function ResetToken() {
  const { info, reload } = useSync();
  const [msg, setMsg] = useState<string | null>(null);
  const reset = async () => {
    if (!confirm("Disconnect the iPhone? Its Shortcut stops uploading until you scan a new setup code.")) return;
    try {
      const r = await api<{ revoked: boolean }>("/sync/token", { method: "DELETE" });
      setMsg(r.revoked ? "Done. The old Shortcut can no longer upload." : "There was no connected iPhone.");
      reload();
    } catch (e) {
      setMsg(explain(e));
    }
  };
  return (
    <>
      {info && <DeviceLine info={info} />}
      <button className="danger" onClick={reset}>Reset my upload token</button>
      {msg && <p className="muted small">{msg}</p>}
    </>
  );
}
