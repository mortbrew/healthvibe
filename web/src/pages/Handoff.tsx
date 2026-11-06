import { useEffect, useRef, useState } from "react";
import { api, ApiError, HaeToken } from "../api";
import { PhoneCopy } from "../components/Hae";

/** /h/<code>: opened on the iPhone from the desktop QR code. No login. */
export default function HandoffPage({ code }: { code: string }) {
  const [tok, setTok] = useState<HaeToken | null>(null);
  const [err, setErr] = useState<"gone" | "busy" | "other" | null>(null);
  const once = useRef(false);
  useEffect(() => {
    if (once.current) return; // React StrictMode runs effects twice; the code is single use.
    once.current = true;
    // Drop the code from the address bar and history.
    history.replaceState(null, "", "/h/");
    api<HaeToken>("/handoff/claim", { method: "POST", body: { code } })
      .then(setTok)
      .catch((e) => setErr(e instanceof ApiError && e.status === 410 ? "gone" : e instanceof ApiError && e.status === 429 ? "busy" : "other"));
  }, [code]);
  return (
    <div className="handoffpage">
      <h1>healthvibe</h1>
      {!tok && !err && <p className="muted">Getting your setup…</p>}
      {err === "gone" && (
        <div className="card">
          <h2>This link has already been used or has expired</h2>
          <p className="muted">For your safety each QR code works once, for 10 minutes. On your computer, open healthvibe and tap <b>Show QR code for my iPhone</b> again.</p>
        </div>
      )}
      {err === "busy" && <div className="card"><p>Too many tries. Wait a few minutes and scan a new QR code.</p></div>}
      {err === "other" && <div className="card"><p>Couldn't reach healthvibe. Check your connection and scan a new QR code.</p></div>}
      {tok && (
        <>
          <p className="lead">In Health Auto Export: <b>Automations → + → REST API</b>. Then copy and paste these three, in order.</p>
          <PhoneCopy url={tok.url} token={tok.token} />
          <p className="muted small">This is shown only once. Paste the header value before you leave this page. Lost it? Make a new QR code on your computer.</p>
          <p className="muted small">Then set Export Format to JSON, turn on all metrics except Heart Rate, and tap Manual Export to test. Your computer shows when data arrives.</p>
        </>
      )}
    </div>
  );
}
