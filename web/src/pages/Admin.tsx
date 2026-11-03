import { useEffect, useState } from "react";
import { api, explain, User } from "../api";

function tempPassword() {
  const words = "amber birch cedar delta ember fjord grove harbor indigo juniper kestrel lumen meadow nimbus orbit".split(" ");
  const r = crypto.getRandomValues(new Uint32Array(4));
  return `${words[r[0] % words.length]}-${words[r[1] % words.length]}-${words[r[2] % words.length]}-${r[3] % 100}`;
}

export default function Admin() {
  const [users, setUsers] = useState<User[]>([]);
  const [err, setErr] = useState<string | null>(null);
  const [created, setCreated] = useState<{ username: string; pw: string } | null>(null);
  const [pw, setPw] = useState(tempPassword);
  const load = () => api<{ users: User[] }>("/admin/users").then((r) => setUsers(r.users));
  useEffect(() => {
    load();
  }, []);
  return (
    <div>
      <header className="pagehead"><h1>Users</h1></header>
      <form
        className="card"
        onSubmit={async (e) => {
          e.preventDefault();
          const d = new FormData(e.currentTarget);
          const form = e.currentTarget;
          setErr(null);
          try {
            const username = String(d.get("username"));
            await api("/admin/users", {
              method: "POST",
              body: { username, temporary_password: pw, is_admin: d.get("admin") === "on" },
            });
            setCreated({ username, pw });
            form.reset();
            setPw(tempPassword());
            load();
          } catch (x) {
            setErr(explain(x));
          }
        }}
      >
        <h2>Add a person</h2>
        <p className="muted">They sign in with the temporary password and must choose their own before they can see anything.</p>
        <div className="row wrap">
          <label>Username<input name="username" required /></label>
          <label>Temporary password<input value={pw} onChange={(e) => setPw(e.target.value)} minLength={12} required /></label>
          <label className="check"><input type="checkbox" name="admin" /> Admin</label>
          <button className="primary">Create user</button>
        </div>
        {err && <p className="error">{err}</p>}
        {created && (
          <p className="ok">
            Created <b>{created.username}</b>. Share the temporary password <code>{created.pw}</code> privately; it's not shown again.
          </p>
        )}
      </form>
      <section className="card">
        <h2>People</h2>
        <table>
          <thead><tr><th>Username</th><th>Role</th><th>Status</th></tr></thead>
          <tbody>
            {users.map((u) => (
              <tr key={u.id}>
                <td>{u.username}</td>
                <td>{u.is_admin ? "Admin" : "Member"}</td>
                <td>{u.must_change_password ? <span className="pill running">temporary password</span> : <span className="pill done">active</span>}</td>
              </tr>
            ))}
          </tbody>
        </table>
        <p className="muted foot">Each person only ever sees their own data.</p>
      </section>
    </div>
  );
}
