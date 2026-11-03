import { useState } from "react";
import { Link } from "../router";

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
        <Link to="/help/shortcut" className="card linkcard">
          <h2>Set up the iPhone Shortcut</h2>
          <p className="muted">Send new Health data automatically every day, step by step.</p>
        </Link>
        <Link to="/help/messed-up" className="card linkcard">
          <h2>Help, I messed up my Shortcut</h2>
          <p className="muted">Reset everything on the phone and reinstall from scratch.</p>
        </Link>
        <Link to="/import" className="card linkcard">
          <h2>Import an export.zip</h2>
          <p className="muted">One-off import of your full history from the Health app.</p>
        </Link>
      </div>
    </div>
  );
}

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
        <b>Preview.</b> The signed Shortcut download, upload tokens and the setup QR codes arrive in a later
        release. Until then, use <Link to="/import">Import</Link> with an <code>export.zip</code>.
      </div>
      <ol className="guide">
        <Step n={1} title="Install the Shortcut" shot="Add Shortcut sheet">
          <p>On your iPhone, scan <b>QR code 1</b> with the Camera app (or open the link on the phone) and tap <b>Add Shortcut</b>.</p>
          <div className="qr placeholder">QR code 1</div>
        </Step>
        <Step n={2} title="Allow Health access" shot="Allow access to Health">
          <p>Run <b>healthvibe</b> once from the Shortcuts app. When asked, tap <b>Allow</b> for each Health type. You can review this later in Settings → Health → Data Access → Shortcuts.</p>
        </Step>
        <Step n={3} title="Connect it to your account" shot="Scan setup code">
          <p>The first run asks you to scan <b>QR code 2</b>. It saves your server address and personal upload token to <code>hv-config.json</code> in iCloud Drive → Shortcuts → healthvibe.</p>
          <div className="qr placeholder">QR code 2</div>
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
          <button className="danger" disabled title="Arrives with Shortcut uploads">Reset my upload token</button>
          <p className="muted small">Available once Shortcut uploads ship.</p>
        </Step>
        <Step n={5} title="Reinstall">
          <p>Scan <b>QR code 1</b> for a fresh signed download, or build and sign the Shortcut yourself on a Mac (see <code>docs/shortcut-build.md</code>).</p>
          <div className="qr placeholder">QR code 1</div>
        </Step>
        <Step n={6} title="Reconfigure">
          <p>Run it once and scan <b>QR code 2</b> (a fresh setup code), then create the automation again with the <Link to="/help/shortcut">setup guide</Link>.</p>
          <div className="qr placeholder">QR code 2</div>
        </Step>
      </ol>
      <p className="muted">Optional: Settings → Health → Data Access → Shortcuts to review or reset Health permissions.</p>
    </div>
  );
}
