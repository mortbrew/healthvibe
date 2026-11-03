import { useSync } from "../components/Sync";
import { Link } from "../router";

/** Hand-build guide for the healthvibe Shortcut, matching the server API. */
function A({ children }: { children: React.ReactNode }) {
  return <b className="action">{children}</b>;
}

function Code({ children }: { children: string }) {
  return <pre className="codeblock">{children}</pre>;
}

export default function BuildGuide() {
  const { info } = useSync();
  const enabled = info ? info.metrics.filter((m) => info.enabled.includes(m.key) && m.kind !== "workout") : [];
  return (
    <div className="help">
      <header className="pagehead">
        <p className="eyebrow"><Link to="/help">Help</Link></p>
        <h1>Build the Shortcut yourself</h1>
      </header>
      <p className="lead">
        This builds the same Shortcut the signed download will contain. Do it on the iPhone in the Shortcuts app
        (or on a Mac; it syncs over iCloud). Words in <A>bold</A> are action names you can type into the action
        search. iOS sometimes names things slightly differently between versions; pick the closest match.
      </p>
      <div className="card notice">
        Before you start: <Link to="/settings">choose what to sync</Link>, and in the <b>Files</b> app create the folder
        {" "}<b>iCloud Drive → Shortcuts → healthvibe</b>.
      </div>

      <ol className="guide">
        <li className="step"><div className="stepnum">0</div><div className="stepbody">
          <h3>Create the Shortcut and its version</h3>
          <ol className="steps">
            <li>Shortcuts → <b>+</b>. Name it <code>healthvibe</code>.</li>
            <li><A>Text</A>: <code>0.1.0</code>, then <A>Set Variable</A> <code>Version</code>.</li>
          </ol>
        </div></li>

        <li className="step"><div className="stepnum">1</div><div className="stepbody">
          <h3>Load the saved settings, or connect on the first run</h3>
          <ol className="steps">
            <li><A>Get File from Folder</A>: folder <b>Shortcuts</b>, path <code>healthvibe/hv-config.json</code>. Show more → turn <b>Error If Not Found</b> off.</li>
            <li><A>If</A> <i>File</i> <b>has any value</b>:
              <ol className="steps">
                <li><A>Get Dictionary from Input</A> (the File), then <A>Get Dictionary Value</A> <code>server</code> → <A>Set Variable</A> <code>Server</code>.</li>
                <li><A>Get Dictionary Value</A> <code>token</code> from the same dictionary → <A>Set Variable</A> <code>Token</code>.</li>
              </ol>
            </li>
            <li><b>Otherwise</b>:
              <ol className="steps">
                <li><A>Scan QR or Barcode</A> → <A>Set Variable</A> <code>Link</code>. (You scan the code on <Link to="/help/shortcut">the setup page</Link>.)</li>
                <li><A>Match Text</A> <code>{"server=([^&]+)"}</code> in <i>Link</i> → <A>Get Group from Matched Text</A> group 1 → <A>URL Encode</A> set to <b>Decode</b> → <A>Set Variable</A> <code>Server</code>.</li>
                <li><A>Match Text</A> <code>{"code=([^&]+)"}</code> in <i>Link</i> → <A>Get Group from Matched Text</A> group 1 → <A>Set Variable</A> <code>Code</code>.</li>
                <li><A>Get Device Details</A>: <b>Device Name</b> → <A>Set Variable</A> <code>DeviceName</code>.</li>
                <li><A>Get Contents of URL</A>: <code>Server</code><code>/api/v1/shortcut/claim</code>, Method <b>POST</b>, Request Body <b>JSON</b> with Text fields
                  {" "}<code>setup_code</code> = <i>Code</i>, <code>device_name</code> = <i>DeviceName</i>, <code>shortcut_version</code> = <i>Version</i>.</li>
                <li><A>Get Dictionary Value</A> <code>upload_token</code> → <A>Set Variable</A> <code>Token</code>.</li>
                <li><A>Dictionary</A> with Text fields <code>server</code> = <i>Server</i>, <code>token</code> = <i>Token</i>, <code>device</code> = <i>DeviceName</i>, <code>configured_at</code> = <i>Current Date</i>.</li>
                <li><A>Save File</A>: the Dictionary, to <b>Shortcuts</b>, subpath <code>healthvibe/hv-config.json</code>, <b>Ask Where to Save</b> off, <b>Overwrite If File Exists</b> on.</li>
              </ol>
            </li>
            <li><b>End If</b>.</li>
          </ol>
          <p className="muted small">The claim answers <code>{"{\"upload_token\": \"hvu_…\", \"config\": {…}}"}</code>. A wrong or used code gives HTTP 401 <code>invalid_or_expired_code</code>; reload the setup page for a new one.</p>
        </div></li>

        <li className="step"><div className="stepnum">2</div><div className="stepbody">
          <h3>Ask the server what to send</h3>
          <ol className="steps">
            <li><A>Get Contents of URL</A>: <code>Server</code><code>/api/v1/shortcut/config</code>, Method <b>GET</b>, Headers
              {" "}<code>Authorization</code> = <code>Bearer </code><i>Token</i> (one space after Bearer) and <code>X-HV-Version</code> = <i>Version</i>.
              → <A>Set Variable</A> <code>HVConfig</code>.</li>
            <li><A>Get Dictionary Value</A> <code>enabled</code> from <i>HVConfig</i> → <A>Combine Text</A> with <b>Comma</b> → <A>Set Variable</A> <code>Enabled</code>.</li>
          </ol>
          <p className="muted small">Example answer:</p>
          <Code>{`{"enabled": ["stepCount", "sleepAnalysis"],
 "since": {"stepCount": "2026-11-02T12:10:00Z", "sleepAnalysis": "2026-10-27T05:00:00Z"},
 "units": {"stepCount": "count"},
 "max_samples_per_metric": 5000, "max_request_bytes": 5242880,
 "notify": "errors_only", "min_shortcut_version": "0.1.0", ...}`}</Code>
          <p className="muted small"><code>since</code> is where each type should start: just before the newest sample the server already has, at most 30 days back, or the last 7 days if it has nothing yet.</p>
        </div></li>

        <li className="step"><div className="stepnum">3</div><div className="stepbody">
          <h3>One block per Health type</h3>
          <p>Repeat this block for every type you chose. Example for <b>Steps</b> (key <code>stepCount</code>):</p>
          <ol className="steps">
            <li><A>If</A> <i>Enabled</i> <b>contains</b> <code>stepCount</code>:</li>
            <li><A>Get Dictionary Value</A> <code>since.stepCount</code> from <i>HVConfig</i> → <A>Get Dates from Input</A> → <A>Set Variable</A> <code>Since</code>.</li>
            <li><A>Find Health Samples</A> where <b>Type</b> is <b>Steps</b> and <b>Start Date</b> <b>is after</b> <i>Since</i>; Sort by <b>Start Date</b>, Oldest First; Limit <b>5000</b>.</li>
            <li><A>Repeat with Each</A> item in Health Samples:
              <ol className="steps">
                <li><A>Get Details of Health Sample</A> <b>Start Date</b> → <A>Format Date</A> <b>ISO 8601</b> (Include Time on) → <A>Set Variable</A> <code>S</code>.</li>
                <li>Same for <b>End Date</b> → <code>E</code>.</li>
                <li><A>Get Details of Health Sample</A> <b>Value</b> → <code>V</code>; <b>Unit</b> → <code>U</code>; <b>Source</b> → <code>Src</code>.</li>
                <li><A>Dictionary</A> with Text fields <code>start</code> = <i>S</i>, <code>end</code> = <i>E</i>, <code>value</code> = <i>V</i>, <code>unit</code> = <i>U</i>, <code>source</code> = <i>Src</i>.</li>
              </ol>
            </li>
            <li><b>End Repeat</b> → <A>Combine Text</A> <i>Repeat Results</i> with Custom separator <code>,</code> → <A>Set Variable</A> <code>Items</code>.</li>
            <li><A>If</A> <i>Items</i> <b>has any value</b>: <A>Text</A> <code>{`{"schema":1,"shortcut_version":"`}</code><i>Version</i><code>{`","metrics":{"stepCount":{"samples":[`}</code><i>Items</i><code>{"]}}}"}</code></li>
            <li><A>Get Contents of URL</A>: <code>Server</code><code>/api/v1/ingest</code>, Method <b>POST</b>, the same two Headers plus <code>Content-Type</code> = <code>application/json</code>, Request Body <b>File</b> = the Text.</li>
            <li><b>End If</b> (twice).</li>
          </ol>
          <p className="muted small">
            Each upload answers <code>{"{\"accepted\": 12, \"duplicates\": 3, \"invalid\": 0, \"rejected\": {}, \"next_since\": {…}}"}</code>.
            Re-sending the same samples is safe: duplicates are skipped. Values may be in your phone's units (lb, mi, °F, Cal); the server converts them.
            Sleep values like <i>Core</i>, <i>Deep</i>, <i>REM</i>, <i>Awake</i>, <i>In Bed</i> are understood as they are.
          </p>
          {enabled.length > 0 && (
            <>
              <p>Your chosen types (Find Health Samples type → key):</p>
              <table>
                <thead><tr><th>Type in Shortcuts</th><th>Key</th><th>Server unit</th></tr></thead>
                <tbody>
                  {enabled.map((m) => (
                    <tr key={m.key}><td>{m.shortcut_type}</td><td><code>{m.key}</code></td><td>{m.unit || "category"}</td></tr>
                  ))}
                </tbody>
              </table>
            </>
          )}
        </div></li>

        <li className="step"><div className="stepnum">4</div><div className="stepbody">
          <h3>Workouts (optional)</h3>
          <p>If you chose Workouts: <A>Find Health Samples</A> of type <b>Workouts</b> after <code>since.workouts</code>, and for each build a Dictionary with
            {" "}<code>type</code> (e.g. <i>Running</i>), <code>start</code>, <code>end</code> (ISO 8601) and <code>duration_s</code>. Send them as a top-level list:</p>
          <Code>{`{"schema":1,"workouts":[{"type":"Running","start":"…","end":"…","duration_s":1800,"source":"Apple Watch"}]}`}</Code>
        </div></li>

        <li className="step"><div className="stepnum">5</div><div className="stepbody">
          <h3>Try it</h3>
          <ol className="steps">
            <li>Tap ▶. Allow Health access for each type, scan the setup code from <Link to="/help/shortcut">the setup page</Link>, and allow the Shortcut to connect to your server.</li>
            <li>Open the <Link to="/">Summary</Link>; new data shows up right away. Run it again: it only sends what's new.</li>
            <li>Then add the automation (step 4 onwards of the <Link to="/help/shortcut">setup guide</Link>).</li>
          </ol>
          <p className="muted small">Errors: 401 = token revoked or wrong (reset and scan a new code); 413 = upload too big (lower the Limit); 422 = unknown key (check the spelling); 426 = Shortcut too old; 429 = too many runs, try later.</p>
        </div></li>
      </ol>
    </div>
  );
}
