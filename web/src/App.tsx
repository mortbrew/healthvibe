import { useCallback, useEffect, useRef, useState } from "react";
import { api, User } from "./api";
import { applyTheme, ThemePrefs, watchSystem } from "./theme";
import { Link, navigate, usePath } from "./router";
import { ChangePassword, Login, Setup } from "./pages/Auth";
import Dashboard from "./pages/Dashboard";
import Import from "./pages/Import";
import Settings from "./pages/Settings";
import Admin from "./pages/Admin";
import { HaeGuide, HelpIndex, MessedUp, ShortcutGuide } from "./pages/Help";
import Onboarding from "./pages/Onboarding";
import BuildGuide from "./pages/Build";
import HandoffPage from "./pages/Handoff";

type Session = { needs_setup: boolean; user: User | null };

const prefsOf = (u: User): ThemePrefs => ({ mode: u.theme_mode, light: u.theme_light, dark: u.theme_dark });

export default function App() {
  const [session, setSession] = useState<Session | null>(null);
  const [error, setError] = useState(false);
  const path = usePath();
  const prefs = useRef<ThemePrefs | null>(null);

  const setUser = useCallback((user: User | null) => {
    setSession((s) => ({ needs_setup: false, ...s, user }));
    if (user) {
      prefs.current = prefsOf(user);
      applyTheme(prefs.current);
    }
  }, []);

  useEffect(() => {
    api<Session>("/session")
      .then((s) => {
        setSession(s);
        if (s.user) {
          prefs.current = prefsOf(s.user);
          applyTheme(prefs.current);
        }
      })
      .catch(() => setError(true));
    watchSystem(() => prefs.current ?? { mode: "system", light: "daylight", dark: "midnight" });
  }, []);

  // Phone handoff link: works without a session.
  if (path.startsWith("/h/")) return <HandoffPage code={path.slice(3)} />;
  if (error) return <Centered><p>Can't reach the healthvibe server.</p></Centered>;
  if (!session) return <Centered><p className="muted">Loading…</p></Centered>;
  if (session.needs_setup) return <Centered><Setup onDone={setUser} /></Centered>;
  if (!session.user) return <Centered><Login onDone={setUser} /></Centered>;
  if (session.user.must_change_password)
    return (
      <Centered>
        <ChangePassword user={session.user} forced onDone={setUser} />
      </Centered>
    );

  const user = session.user;
  const logout = async () => {
    await api("/logout", { method: "POST" }).catch(() => {});
    setSession({ needs_setup: false, user: null });
    navigate("/");
  };

  let page: React.ReactNode;
  if (path === "/welcome" || (!user.onboarding_done && (path === "/" || path === "/dashboard")))
    page = <Onboarding user={user} onUser={setUser} />;
  else if (path === "/" || path === "/dashboard") page = <Dashboard />;
  else if (path === "/import") page = <Import />;
  else if (path === "/settings") page = <Settings user={user} onUser={setUser} />;
  else if (path === "/admin" && user.is_admin) page = <Admin />;
  else if (path === "/help") page = <HelpIndex />;
  else if (path === "/help/health-auto-export") page = <HaeGuide />;
  else if (path === "/help/shortcut") page = <ShortcutGuide />;
  else if (path === "/help/messed-up") page = <MessedUp />;
  else if (path === "/help/build") page = <BuildGuide />;
  else page = <div className="card empty"><h2>Not found</h2><Link to="/">Back to the dashboard</Link></div>;

  const nav = [
    ["/", "Summary", "◎"],
    ["/import", "Import", "⇪"],
    ["/help", "Help", "?"],
    ["/settings", "Settings", "⚙"],
    ...(user.is_admin ? [["/admin", "Users", "👥"]] : []),
  ];
  const active = (to: string) => (to === "/" ? path === "/" || path === "/dashboard" : path.startsWith(to));

  return (
    <div className="shell">
      <nav className="nav">
        <div className="brand">
          <span className="logo" aria-hidden>
            <i style={{ borderColor: "var(--move)" }} />
            <i style={{ borderColor: "var(--exercise)" }} />
            <i style={{ borderColor: "var(--stand)" }} />
          </span>
          healthvibe
        </div>
        {nav.map(([to, label, icon]) => (
          <Link key={to} to={to} className={active(to) ? "navlink active" : "navlink"}>
            <span className="icon" aria-hidden>{icon}</span>
            <span>{label}</span>
          </Link>
        ))}
        <div className="spacer" />
        <div className="who">
          <span className="muted">{user.username}</span>
          <button className="ghost small" onClick={logout}>Sign out</button>
        </div>
      </nav>
      <main className="main">{page}</main>
    </div>
  );
}

function Centered({ children }: { children: React.ReactNode }) {
  return <div className="centered">{children}</div>;
}
