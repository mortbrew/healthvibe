import { useCallback, useEffect, useRef, useState } from "react";
import { api, Dashboard, explain, User } from "./api";
import { applyTheme, ThemePrefs, watchSystem } from "./theme";
import { Link, navigate, usePath } from "./router";
import { ChangePassword, Login, Setup } from "./pages/Auth";
import Summary, { ActivityDay, DistanceDetail, StepsDetail, WorkoutDetail } from "./health/Summary";
import { Board } from "./health/Board";
import Browse, { AllRecords, BrowseCategory, MetricDetail } from "./health/Browse";
import Fitness, { FitnessSessions } from "./health/Fitness";
import Trends, { TrendDetail } from "./health/Trends";
import { AccountMenu, AccountSheet, AppearancePage, DetailsPage, ImportPage, PasswordPage, PrivacyPage,
  GoalsPage, ProfilePage, SyncPage, UnitsPage, UsersPage, useProfile } from "./health/Account";
import { EMPTY_PROFILE, HealthCtx, unitsOf } from "./health/ctx";
import { asCfg } from "./health/avatar";
import { BP, DownIcon, GripIcon, MinusIcon, UpIcon, useMinWidth, XIcon } from "./health/ui";
import { mobileDestinations, moveItem } from "./health/nav-layout";
import { useGripReorder } from "./health/reorder";
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
  else if (seg[0] === "workouts" && seg[1]) page = <WorkoutDetail id={seg[1]} />;
  else if (path === "/summary/trends" || path === "/dashboards/trends") page = <Trends />;
  else if ((seg[0] === "summary" || seg[0] === "dashboards") && seg[1] === "trends" && seg[2]) page = <TrendDetail k={seg[2]} />;
  else if (seg[0] === "activity" && /^\d{4}-\d{2}-\d{2}$/.test(seg[1] ?? "")) page = <ActivityDay date={seg[1]} />;
  else if (path === "/fitness") page = <Fitness />;
  else if (path === "/fitness/sessions") page = <FitnessSessions />;
  else if (path === "/health" || path === "/browse") page = <Browse />;
  else if ((seg[0] === "health" || seg[0] === "browse") && seg[1] === "metric" && seg[2] && seg[3] === "all") page = <AllRecords metric={dec(seg[2])} />;
  else if ((seg[0] === "health" || seg[0] === "browse") && seg[1] === "metric" && seg[2]) page = <MetricDetail metric={dec(seg[2])} />;
  else if ((seg[0] === "health" || seg[0] === "browse") && seg[1]) page = <BrowseCategory k={seg[1]} />;
  else if (seg[0] === "d" && /^\d+$/.test(seg[1] ?? "")) page = <Board which={Number(seg[1])} />;
  else if (path === "/dashboards" || path === "/dashboard") page = <Summary />;
  else if (path === "/account" || path === "/settings") { page = <Summary />; sheet = <AccountSheet />; }
  else if (path === "/account/profile") page = <ProfilePage />;
  else if (path === "/account/details") page = <DetailsPage />;
  else if (path === "/account/units") page = <UnitsPage />;
  else if (path === "/account/goals") page = <GoalsPage />;
  else if (path === "/account/privacy") page = <PrivacyPage />;
  else if (path === "/account/sync") page = <SyncPage />;
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

  const active = (to: string) => {
    if (to === "/") return path === "/" || path === "/summary" || path.startsWith("/summary/trends");
    if (to === "/fitness") return path.startsWith("/fitness") || path.startsWith("/activity") || path.startsWith("/workouts") || path.startsWith("/summary/steps") || path.startsWith("/summary/distance");
    if (to === "/health") return path.startsWith("/health") || path.startsWith("/browse");
    if (to.startsWith("/d/")) return path === to || path.startsWith(`${to}/`);
    return path === to || path.startsWith(`${to}/`);
  };

  return (
    <HealthCtx.Provider value={ctx}>
      <div className={/^\/(account|settings)\//.test(path) ? "app no-tabs" : "app"}>
        <PrimaryNav path={path} active={active} acctPath={acctPath} avatar={avatar} name={ctx.name} />
        <main className="main">{page}</main>
        {sheet}
      </div>
    </HealthCtx.Provider>
  );
}

type NavItem = { kind: "summary" | "fitness" | "health" | "dashboard" | "add"; id?: number };

function PrimaryNav({ path, active, acctPath, avatar, name }: { path: string; active: (to: string) => boolean; acctPath: boolean; avatar: ReturnType<typeof asCfg>; name: string }) {
  const wide = useMinWidth(BP.tablet);
  const [items, setItems] = useState<NavItem[] | null>(null);
  const [boards, setBoards] = useState<Dashboard[]>([]);
  const [editing, setEditing] = useState(false);
  const [more, setMore] = useState(false);
  const [pendingDelete, setPendingDelete] = useState<number | null>(null);
  const [renaming, setRenaming] = useState<number | null>(null);
  const [rename, setRename] = useState("");
  const [navErr, setNavErr] = useState("");
  const load = () => {
    api<{ items: NavItem[] }>("/nav").then((r) => setItems(r.items), () => setItems([]));
    api<{ dashboards: Dashboard[] }>("/dashboards").then((r) => setBoards(r.dashboards), () => {});
  };
  useEffect(load, [path]);
  const save = async (next: NavItem[]) => {
    setNavErr("");
    setItems(next);
    try {
      const r = await api<{ items: NavItem[] }>("/nav", { method: "PUT", body: { items: next } });
      setItems(r.items);
    } catch (e) {
      setNavErr(explain(e));
      load();
    }
  };
  const move = (from: number, to: number) => items && save(moveItem(items, from, to));
  const grip = useGripReorder(move);
  const create = async () => {
    setNavErr("");
    try {
      const n = boards.filter((b) => !b.builtin).length + 1;
      const d = (await api<{ dashboard: Dashboard }>("/dashboards", { method: "POST", body: { name: n === 1 ? "Dashboard" : `Dashboard ${n}`, widgets: [] } })).dashboard;
      setMore(false);
      setEditing(false);
      load();
      navigate(`/d/${d.id}`);
    } catch (e) {
      setNavErr(explain(e));
    }
  };
  const remove = async (id: number) => {
    await api(`/dashboards/${id}`, { method: "DELETE" });
    setPendingDelete(null);
    if (path === `/d/${id}`) navigate("/");
    load();
  };
  const commitRename = async (id: number) => {
    const name = rename.trim();
    if (name) await api(`/dashboards/${id}`, { method: "PUT", body: { name } });
    setRenaming(null);
    load();
  };
  const labelOf = (item: NavItem) => {
    if (item.kind === "summary") return "Summary";
    if (item.kind === "fitness") return "Fitness";
    if (item.kind === "health") return "Health";
    if (item.kind === "add") return "+ Dashboard";
    return boards.find((b) => b.id === item.id)?.name ?? "Dashboard";
  };
  const hrefOf = (item: NavItem) => item.kind === "summary" ? "/" : item.kind === "fitness" ? "/fitness" : item.kind === "health" ? "/health" : item.kind === "dashboard" ? `/d/${item.id}` : "";
  const list = items ?? [];
  const activeItem = list.find((item) => item.kind !== "add" && active(hrefOf(item))) ?? null;
  const phoneTabs = mobileDestinations(list, activeItem, 3);
  const controls = (item: NavItem, i: number) => {
    const label = labelOf(item);
    return (
      <span className="nav-moves">
        <button type="button" className="iconbtn grip" aria-label={`Drag to reorder ${label}`} {...grip.gripProps(i)}><GripIcon /></button>
        <button type="button" className="iconbtn" aria-label={`Move ${label} up`} disabled={i === 0} onClick={() => move(i, i - 1)}><UpIcon /></button>
        <button type="button" className="iconbtn" aria-label={`Move ${label} down`} disabled={i === list.length - 1} onClick={() => move(i, i + 1)}><DownIcon /></button>
        {item.kind === "dashboard" && item.id != null && <button type="button" className="iconbtn" aria-label={`Rename ${label}`} onClick={() => { setRenaming(item.id!); setRename(label); }}>Aa</button>}
        {item.kind === "dashboard" && item.id != null && <button type="button" className="iconbtn" aria-label={`Delete ${label}`} onClick={() => setPendingDelete(item.id!)}><MinusIcon /></button>}
      </span>
    );
  };
  return (
    <>
      <nav className="sidenav" aria-label="Primary">
        <div className="side-top">
          <Link to="/" className="brand"><span className="logo" aria-hidden><i style={{ borderColor: "var(--move)" }} /><i style={{ borderColor: "var(--exercise)" }} /><i style={{ borderColor: "var(--stand)" }} /></span>healthvibe</Link>
          <button type="button" className="link" onClick={() => setEditing((v) => !v)}>{editing ? "Done" : "Edit"}</button>
        </div>
        {navErr && <p className="muted small side-err">{navErr}</p>}
        <div className="sidenav-links">
          {list.map((item, i) => {
            const label = labelOf(item);
            if (editing) {
              return (
                <div key={`${item.kind}-${item.id ?? i}`} className="sidelink edit-nav-row" {...grip.rowProps(i)}>
                  <span>{label}</span>
                  {controls(item, i)}
                </div>
              );
            }
            if (item.kind === "add") {
              return <button key="add" type="button" className="sidelink add-dash" onClick={create}>{label}</button>;
            }
            const to = hrefOf(item);
            return <Link key={to} to={to} className={active(to) ? "sidelink active" : "sidelink"} aria-current={active(to) ? "page" : undefined}><span>{label}</span></Link>;
          })}
        </div>
        <Link to="/account" className={acctPath ? "sideacct active" : "sideacct"} aria-label={`Account for ${name}`}><AvatarSvg cfg={avatar} size={40} /><span><b>{name}</b><small className="muted">Account</small></span></Link>
      </nav>
      <nav className="tabpill" aria-label="Tabs">
        {phoneTabs.map((item) => {
          const to = hrefOf(item);
          const label = labelOf(item);
          return <Link key={to} to={to} className={active(to) ? "tab active" : "tab"} aria-current={active(to) ? "page" : undefined}><span>{label}</span></Link>;
        })}
        <button type="button" className="tab" aria-haspopup="dialog" aria-expanded={more} onClick={() => setMore(true)}>More</button>
      </nav>
      {more && (
        <div className="sheet-backdrop" onClick={() => setMore(false)}>
          <div className="sheet" role="dialog" aria-label="More destinations" onClick={(e) => e.stopPropagation()}>
            <div className="sheet-head">
              <button type="button" className="roundbtn" aria-label="Close" onClick={() => setMore(false)}><XIcon /></button>
              <b>More</b>
              <span style={{ width: 38 }} />
            </div>
            <div className="sheet-body">
              {navErr && <p className="muted">{navErr}</p>}
              <div className="group">
              {list.map((item, i) => {
                const label = labelOf(item);
                if (item.kind === "add") {
                  return <button key="add" type="button" className="row" onClick={create}><span className="row-label">{label}</span></button>;
                }
                const to = hrefOf(item);
                return <button key={`${item.kind}-${item.id ?? i}`} type="button" className="row" aria-current={active(to) ? "page" : undefined} onClick={() => { setMore(false); navigate(to); }}><span className="row-label">{label}</span></button>;
              })}
              </div>
              <button type="button" className="pill" onClick={() => { setMore(false); setEditing(true); }}>Edit</button>
            </div>
          </div>
        </div>
      )}
      {!wide && editing && (
        <div className="sheet-backdrop" onClick={() => setEditing(false)}>
          <div className="sheet" role="dialog" aria-label="Edit navigation" onClick={(e) => e.stopPropagation()}>
            <div className="sheet-head">
              <button type="button" className="roundbtn" aria-label="Close" onClick={() => setEditing(false)}><XIcon /></button>
              <b>Edit</b>
              <span style={{ width: 38 }} />
            </div>
            <div className="sheet-body">
              {navErr && <p className="muted">{navErr}</p>}
              {list.map((item, i) => {
                const label = labelOf(item);
                return (
                  <div key={`${item.kind}-${item.id ?? i}`} className="row static" {...grip.rowProps(i)}>
                    <span className="row-label">{label}</span>
                    {controls(item, i)}
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      )}
      {pendingDelete != null && (
        <div className="sheet-backdrop" onClick={() => setPendingDelete(null)}>
          <div className="sheet" role="dialog" aria-label="Delete dashboard" onClick={(e) => e.stopPropagation()}>
            <div className="sheet-body">
              <p>Delete this dashboard? Your health data stays.</p>
              <div className="row">
                <button type="button" className="pill red" onClick={() => remove(pendingDelete)}>Delete</button>
                <button type="button" className="pill" onClick={() => setPendingDelete(null)}>Cancel</button>
              </div>
            </div>
          </div>
        </div>
      )}
      {renaming != null && (
        <div className="sheet-backdrop" onClick={() => setRenaming(null)}>
          <div className="sheet" role="dialog" aria-label="Rename dashboard" onClick={(e) => e.stopPropagation()}>
            <div className="sheet-body">
              <label>Name<input value={rename} onChange={(e) => setRename(e.target.value)} maxLength={60} /></label>
              <button type="button" className="pill" onClick={() => commitRename(renaming)}>Save</button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}

function Centered({ children }: { children: React.ReactNode }) {
  return <div className="centered">{children}</div>;
}
