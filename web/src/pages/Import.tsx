import { useEffect, useState } from "react";
import { api, explain } from "../api";
import { Link } from "../router";

type Job = {
  id: number;
  status: "running" | "done" | "failed";
  counts: Record<string, { inserted: number; duplicates: number }> | null;
  error: string | null;
  created_at: string;
};

const short = (t: string) => t.replace(/^HK(Quantity|Category)TypeIdentifier/, "");

export default function Import() {
  const [jobs, setJobs] = useState<Job[]>([]);
  const [progress, setProgress] = useState<number | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const load = () => api<{ imports: Job[] }>("/imports").then((r) => setJobs(r.imports));
  useEffect(() => {
    load();
  }, []);
  useEffect(() => {
    if (!jobs.some((j) => j.status === "running")) return;
    const t = setTimeout(load, 1500);
    return () => clearTimeout(t);
  }, [jobs]);

  const upload = (file: File) => {
    setErr(null);
    // XHR (not fetch) for upload progress on large exports.
    const fd = new FormData();
    fd.append("file", file);
    const xhr = new XMLHttpRequest();
    xhr.open("POST", "/api/v1/import");
    xhr.upload.onprogress = (e) => e.lengthComputable && setProgress(e.loaded / e.total);
    xhr.onload = () => {
      setProgress(null);
      if (xhr.status === 200) load();
      else setErr(explain({ code: JSON.parse(xhr.responseText || "{}").error ?? "upload_failed" }));
    };
    xhr.onerror = () => {
      setProgress(null);
      setErr("The upload failed. Try again.");
    };
    xhr.send(fd);
  };

  return (
    <div>
      <header className="pagehead">
        <h1>Import</h1>
      </header>
      <section className="card">
        <h2>Upload an Apple Health export</h2>
        <ol className="steps">
          <li>On iPhone, open <b>Health</b> → tap your picture → <b>Export All Health Data</b>.</li>
          <li>Save <code>export.zip</code> to Files (or AirDrop it to your computer).</li>
          <li>Choose it below. Re-importing is safe: readings already stored are skipped.</li>
        </ol>
        <label className="drop">
          <input type="file" accept=".zip,application/zip" disabled={progress !== null}
            onChange={(e) => e.target.files?.[0] && upload(e.target.files[0])} />
          {progress === null ? "Choose export.zip…" : `Uploading… ${Math.round(progress * 100)}%`}
        </label>
        {progress !== null && <progress value={progress} max={1} />}
        {err && <p className="error">{err}</p>}
        <p className="muted foot">
          Prefer automatic daily syncing? See the <Link to="/help/shortcut">iPhone Shortcut guide</Link>.
        </p>
      </section>
      <section className="card">
        <h2>Recent imports</h2>
        {jobs.length === 0 ? (
          <p className="muted">Nothing imported yet.</p>
        ) : (
          jobs.map((j) => <JobRow key={j.id} job={j} />)
        )}
      </section>
    </div>
  );
}

function JobRow({ job }: { job: Job }) {
  const rows = Object.entries(job.counts ?? {}).sort((a, b) => b[1].inserted - a[1].inserted);
  const total = rows.reduce((a, [, c]) => a + c.inserted, 0);
  const dup = rows.reduce((a, [, c]) => a + c.duplicates, 0);
  return (
    <details className="job">
      <summary>
        <span className={`pill ${job.status}`}>{job.status}</span>
        <span>{new Date(job.created_at).toLocaleString()}</span>
        {job.status === "done" && (
          <span className="muted">{total.toLocaleString()} new · {dup.toLocaleString()} already stored</span>
        )}
        {job.error && <span className="error">{job.error}</span>}
      </summary>
      {rows.length > 0 && (
        <table>
          <thead><tr><th>Type</th><th>New</th><th>Already stored</th></tr></thead>
          <tbody>
            {rows.map(([t, c]) => (
              <tr key={t}><td>{short(t)}</td><td>{c.inserted.toLocaleString()}</td><td>{c.duplicates.toLocaleString()}</td></tr>
            ))}
          </tbody>
        </table>
      )}
    </details>
  );
}
