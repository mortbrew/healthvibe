import { useState } from "react";
import { api, explain, User } from "../api";

function Form(props: {
  title: string;
  intro?: React.ReactNode;
  submit: string;
  onSubmit: (data: FormData) => Promise<void>;
  children: React.ReactNode;
}) {
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  return (
    <form
      className="card auth"
      onSubmit={async (e) => {
        e.preventDefault();
        setBusy(true);
        setErr(null);
        try {
          await props.onSubmit(new FormData(e.currentTarget));
        } catch (x) {
          setErr(explain(x));
        } finally {
          setBusy(false);
        }
      }}
    >
      <h1>{props.title}</h1>
      {props.intro}
      {props.children}
      {err && <p className="error" role="alert">{err}</p>}
      <button className="primary" disabled={busy}>{busy ? "…" : props.submit}</button>
    </form>
  );
}

const s = (d: FormData, k: string) => String(d.get(k) ?? "");

export function Setup({ onDone }: { onDone: (u: User) => void }) {
  return (
    <Form
      title="Welcome to healthvibe"
      intro={<p className="muted">Create the first account. It becomes the administrator and can add other people later.</p>}
      submit="Create admin account"
      onSubmit={async (d) => {
        if (s(d, "password") !== s(d, "confirm")) throw new Error("mismatch");
        const r = await api("/setup", { method: "POST", body: { username: s(d, "username"), password: s(d, "password") } });
        onDone(r.user);
      }}
    >
      <label>Username<input name="username" autoComplete="username" required /></label>
      <label>Password <small className="muted">(12+ characters)</small>
        <input name="password" type="password" autoComplete="new-password" minLength={12} required />
      </label>
      <label>Confirm password<input name="confirm" type="password" autoComplete="new-password" minLength={12} required /></label>
    </Form>
  );
}

export function Login({ onDone }: { onDone: (u: User) => void }) {
  return (
    <Form
      title="Sign in"
      submit="Sign in"
      onSubmit={async (d) => {
        const r = await api("/login", { method: "POST", body: { username: s(d, "username"), password: s(d, "password") } });
        onDone(r.user);
      }}
    >
      <label>Username<input name="username" autoComplete="username" required autoFocus /></label>
      <label>Password<input name="password" type="password" autoComplete="current-password" required /></label>
    </Form>
  );
}

export function ChangePassword(props: { user: User; forced?: boolean; onDone: (u: User) => void }) {
  const [ok, setOk] = useState(false);
  return (
    <Form
      title={props.forced ? "Choose your own password" : "Change password"}
      intro={
        props.forced ? (
          <p className="muted">
            Hi {props.user.username}, you signed in with a temporary password. Pick a new one to continue.
          </p>
        ) : ok ? <p className="ok">Password changed.</p> : null
      }
      submit="Save password"
      onSubmit={async (d) => {
        if (s(d, "new") !== s(d, "confirm")) throw new Error("mismatch");
        const r = await api("/me/password", { method: "POST", body: { current: s(d, "current"), new: s(d, "new") } });
        setOk(true);
        props.onDone(r.user);
      }}
    >
      <label>{props.forced ? "Temporary password" : "Current password"}
        <input name="current" type="password" autoComplete="current-password" required />
      </label>
      <label>New password <small className="muted">(12+ characters)</small>
        <input name="new" type="password" autoComplete="new-password" minLength={12} required />
      </label>
      <label>Confirm new password<input name="confirm" type="password" autoComplete="new-password" minLength={12} required /></label>
    </Form>
  );
}
