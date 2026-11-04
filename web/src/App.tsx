import { useCallback, useEffect, useRef, useState } from "react";
import { api, User } from "./api";
import { applyTheme, ThemePrefs, watchSystem } from "./theme";
import { Link, navigate, usePath } from "./router";
import { ChangePassword, Login, Setup } from "./pages/Auth";
import Dashboard from "./pages/Dashboard";
import Summary, { ActivityDay, DistanceDetail, StepsDetail } from "./health/Summary";
import Browse, { AllRecords, BrowseCategory, MetricDetail } from "./health/Browse";
import Awards from "./health/Awards";
import Trends, { TrendDetail } from "./health/Trends";
import { AccountMenu, AccountSheet, AppearancePage, AwardsSetting, DetailsPage, ImportPage, PasswordPage, PrivacyPage, ProfilePage, SyncPage, UnitsPage, UsersPage, useProfile } from "./health/Account";
import { EMPTY_PROFILE, HealthCtx, unitsOf } from "./health/ctx";
import { asCfg } from "./health/avatar";
import { BP, useMinWidth } from "./health/ui";
import { AvatarSvg } from "./health/avatar";
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
  const desktop = useMinWidth(BP.desktop);
  const [profile, setProfile] = useProfile(session?.user ?? null);

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
    watchSystem(() => prefs.current ?? { mode: "system", light: "health", dark: "health" });
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

  if (!profile) return <Centered><p className="muted">Loading…</p></Centered>;
  const avatar = asCfg(profile?.avatar, user.username);
  const prof = profile ?? EMPTY_PROFILE;
  const ctx = { user, onUser: setUser, profile: prof, setProfile, avatar, name: prof.display_name || user.username, units: unitsOf(prof), logout };
  const seg = path.split("/").filter(Boolean);
  const dec = (x: string) => decodeURIComponent(x);

  let page: React.ReactNode;
  let sheet: React.ReactNode = null;
  if (path === "/welcome" || (!user.onboarding_done && path === "/")) page = <Onboarding user={user} onUser={setUser} />;
  else if (path === "/" || path === "/summary") page = <Summary />;
  else if (path === "/summary/steps") page = <StepsDetail />;
  else if (path === "/summary/distance") page = <DistanceDetail />;
  else if (path === "/summary/trends") page = <Trends />;
  else if (seg[0] === "summary" && seg[1] === "trends" && seg[2]) page = <TrendDetail k={seg[2]} />;
  else if (seg[0] === "activity" && /^\d{4}-\d{2}-\d{2}$/.test(seg[1] ?? "")) page = <ActivityDay date={seg[1]} />;
  else if (path === "/browse") page = <Browse />;
  else if (seg[0] === "browse" && seg[1] === "metric" && seg[2] && seg[3] === "all") page = <AllRecords metric={dec(seg[2])} />;
  else if (seg[0] === "browse" && seg[1] === "metric" && seg[2]) page = <MetricDetail metric={dec(seg[2])} />;
  else if (seg[0] === "browse" && seg[1]) page = <BrowseCategory k={seg[1]} />;
  else if (path === "/dashboards" || path === "/dashboard") page = <Dashboard />;
  else if (path === "/awards") page = <Awards />;
  else if (path === "/account" || path === "/settings") { page = <Summary />; sheet = <AccountSheet />; }
  else if (path === "/account/profile") page = <ProfilePage />;
  else if (path === "/account/details") page = <DetailsPage />;
  else if (path === "/account/units") page = <UnitsPage />;
  else if (path === "/account/privacy") page = <PrivacyPage />;
  else if (path === "/account/sync") page = <SyncPage />;
  else if (path === "/account/awards") page = <AwardsSetting />;
  else if (path === "/account/appearance") page = <AppearancePage />;
  else if (path === "/account/password") page = <PasswordPage />;
  else if (path === "/settings/import" || path === "/import") page = <ImportPage />;
  else if ((path === "/settings/users" || path === "/admin") && user.is_admin) page = <UsersPage />;
  else if (path === "/help") page = <HelpIndex />;
  else if (path === "/help/health-auto-export") page = <HaeGuide />;
  else if (path === "/help/shortcut") page = <ShortcutGuide />;
  else if (path === "/help/messed-up") page = <MessedUp />;
  else if (path === "/help/build") page = <BuildGuide />;
  else page = <div className="card empty"><h2>Not found</h2><Link to="/">Back to Summary</Link></div>;

  // Desktop: Account and Settings are one two-pane page (list left, section right) instead of a sheet + drill-in.
  const acctPath = /^\/(account|settings)(\/|$)/.test(path) || path === "/import" || path === "/admin";
  if (desktop && acctPath) {
    if (sheet) { sheet = null; page = <ProfilePage />; }
    page = (
      <div className="acct2">
        <aside className="acct2-menu" aria-label="Account sections"><h1>Account</h1><AccountMenu /></aside>
        <div className="acct2-pane">{page}</div>
      </div>
    );
  }

  const tabs: [string, string, React.ReactNode][] = [
    ["/", "Summary", <TabIcon key="s" d="M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18Zm0 4a5 5 0 1 1 0 10 5 5 0 0 1 0-10Z" />],
    ["/browse", "Browse", <TabIcon key="b" d="M4 5h16v3H4zM4 10.5h16v3H4zM4 16h16v3H4z" />],
    ["/dashboards", "Dashboards", <TabIcon key="d" d="M4 4h7v9H4zM13 4h7v5h-7zM13 11h7v9h-7zM4 15h7v5H4z" />],
  ];
  const active = (to: string) =>
    to === "/" ? !acctPath && !["/browse", "/dashboards", "/dashboard", "/help"].some((p) => path.startsWith(p)) : path.startsWith(to);

  return (
    <HealthCtx.Provider value={ctx}>
      <div className={/^\/(account|settings)\//.test(path) ? "app no-tabs" : "app"}>
        <nav className="sidenav" aria-label="Primary">
          <Link to="/" className="brand"><span className="logo" aria-hidden><i style={{ borderColor: "var(--move)" }} /><i style={{ borderColor: "var(--exercise)" }} /><i style={{ borderColor: "var(--stand)" }} /></span>healthvibe</Link>
          <div className="sidenav-links">{tabs.map(([to, label, icon]) => <Link key={to} to={to} className={active(to) ? "sidelink active" : "sidelink"} aria-current={active(to) ? "page" : undefined}>{icon}<span>{label}</span></Link>)}</div>
          <Link to="/account" className={acctPath ? "sideacct active" : "sideacct"} aria-label={`Account for ${ctx.name}`}><AvatarSvg cfg={avatar} size={40} /><span><b>{ctx.name}</b><small className="muted">Account</small></span></Link>
        </nav>
        <main className="main">{page}</main>
        <nav className="tabpill" aria-label="Tabs">
          {tabs.map(([to, label, icon]) => (
            <Link key={to} to={to} className={active(to) ? "tab active" : "tab"}>{icon}<span>{label}</span></Link>
          ))}
        </nav>
        {sheet}
      </div>
    </HealthCtx.Provider>
  );
}

function TabIcon({ d }: { d: string }) {
  return <svg width="24" height="24" viewBox="0 0 24 24" aria-hidden><path d={d} fill="currentColor" fillRule="evenodd" /></svg>;
}

function Centered({ children }: { children: React.ReactNode }) {
  return <div className="centered">{children}</div>;
}
