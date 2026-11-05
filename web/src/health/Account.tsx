// Account sheet and its subpages (profile, goals, units, privacy, sync, …).
import { useEffect, useState } from "react";
import { api, explain, User } from "../api";
import { navigate } from "../router";
import { ChangePassword } from "../pages/Auth";
import { Appearance } from "../pages/Settings";
import { HaeSettings } from "../components/Hae";
import { SyncSettings } from "../components/Sync";
import Import from "../pages/Import";
import Admin from "../pages/Admin";
import { AvatarBuilder, AvatarSvg, asCfg } from "./avatar";
import { EMPTY_PROFILE, Profile, unitsOf, Units, useHealth } from "./ctx";
import { Group, NavBar, RoundBtn, Row, XIcon } from "./ui";

export function AccountSheet() {
  return (
    <div className="fullsheet account" role="dialog" aria-label="Account">
      <div className="sheet-head">
        <RoundBtn label="Close" onClick={() => navigate("/")}><XIcon /></RoundBtn>
        <b>Account</b>
        <span style={{ width: 36 }} />
      </div>
      <div className="sheet-body"><AccountMenu /></div>
    </div>
  );
}

/** The Account list: a sheet on phone/tablet, the left pane of the two-pane settings page on desktop. */
export function AccountMenu() {
  const { user, name, avatar, logout } = useHealth();
  return (
    <>
      <Group>
        <Row to="/account/profile" icon={<AvatarSvg cfg={avatar} size={54} />} label={<span className="profrow"><b>{name}</b><small className="muted">@{user.username}</small></span>} />
      </Group>
      <Group>
        <Row to="/account/details" label="Health Details" />
        <Row to="/account/units" label="Units of Measure" />
        <Row to="/account/privacy" label="Privacy" />
      </Group>
      <Group>
        <Row to="/account/sync" label="iPhone Sync (Health Auto Export)" tone="green" />
        <Row to="/settings/import" label="Import Health Data" tone="green" />
        <Row to="/awards" label="Awards" tone="green" />
      </Group>
      {user.is_admin && <Group><Row to="/settings/users" label="Users" /></Group>}
      <Group>
        <Row to="/account/appearance" label="Appearance" />
        <Row to="/help" label="Help" />
        <Row to="/account/password" label="Change Password" />
      </Group>
      <Group><Row onClick={logout} label="Sign Out" tone="red" /></Group>
    </>
  );
}

function Sub({ title, children, wide }: { title: string; children: React.ReactNode; wide?: boolean }) {
  return (
    <div className={wide ? "page sub wide" : "page sub"}>
      <NavBar to="/account" title={title} />
      {children}
    </div>
  );
}

function useSaveProfile() {
  const { profile, setProfile } = useHealth();
  const [msg, setMsg] = useState("");
  const save = async (p: Partial<Profile>) => {
    setMsg("");
    try {
      const r = await api<{ profile: Profile }>("/profile", { method: "PUT", body: { ...profile, ...p } });
      setProfile(r.profile);
      setMsg("Saved.");
    } catch (e) { setMsg(explain(e)); }
  };
  return { profile, save, msg };
}

export function ProfilePage() {
  const { user } = useHealth();
  const { profile, save, msg } = useSaveProfile();
  const [name, setName] = useState(profile.display_name ?? "");
  const [cfg, setCfg] = useState(asCfg(profile.avatar, user.username));
  return (
    <Sub title="Profile & Avatar" wide>
      <section className="card">
        <label className="field">Display name<input value={name} maxLength={60} placeholder={user.username} onChange={(e) => setName(e.target.value)} /></label>
        <p className="muted small">Used in greetings like Trends. Leave blank to use your username.</p>
      </section>
      <h2>Avatar</h2>
      <section className="card"><AvatarBuilder value={cfg} onChange={setCfg} /></section>
      <div className="actions"><button className="pill green" onClick={() => save({ display_name: name.trim() || null, avatar: cfg })}>Save</button>{msg && <span className="muted">{msg}</span>}</div>
    </Sub>
  );
}

export function DetailsPage() {
  const { profile, save, msg } = useSaveProfile();
  const [p, setP] = useState(profile);
  const num = (v: string) => (v === "" ? null : Number(v));
  return (
    <Sub title="Health Details">
      <section className="card form">
        <label className="field">Date of Birth<input type="date" value={p.birth_date ?? ""} onChange={(e) => setP({ ...p, birth_date: e.target.value || null })} /></label>
        <label className="field">Sex<select value={p.sex ?? ""} onChange={(e) => setP({ ...p, sex: e.target.value || null })}><option value="">Not set</option><option value="female">Female</option><option value="male">Male</option><option value="other">Other</option></select></label>
        <label className="field">Height (cm)<input type="number" min={30} max={272} value={p.height_cm ?? ""} onChange={(e) => setP({ ...p, height_cm: num(e.target.value) })} /></label>
        <label className="field check"><input type="checkbox" checked={!!p.wheelchair} onChange={(e) => setP({ ...p, wheelchair: e.target.checked })} />Wheelchair user</label>
      </section>
      <div className="actions"><button className="pill green" onClick={() => save(p)}>Save</button>{msg && <span className="muted">{msg}</span>}</div>
    </Sub>
  );
}

export function UnitsPage() {
  const { profile, save, msg } = useSaveProfile();
  const [u, setU] = useState<Units>(unitsOf(profile));
  const opts: [keyof Units, string, [string, string][]][] = [
    ["distance", "Distance", [["mi", "Miles"], ["km", "Kilometers"]]],
    ["weight", "Weight", [["lb", "Pounds"], ["kg", "Kilograms"]]],
    ["energy", "Energy", [["cal", "Calories"], ["kj", "Kilojoules"]]],
    ["temp", "Temperature", [["f", "°F"], ["c", "°C"]]],
  ];
  return (
    <Sub title="Units of Measure">
      {opts.map(([k, l, os]) => (
        <Group key={k} title={l}>
          {os.map(([v, lab]) => <Row key={v} onClick={() => setU({ ...u, [k]: v })} label={lab} value={u[k] === v ? "✓" : ""} chevron={false} />)}
        </Group>
      ))}
      <div className="actions"><button className="pill green" onClick={() => save({ units: u })}>Save</button>{msg && <span className="muted">{msg}</span>}</div>
    </Sub>
  );
}

export function PrivacyPage() {
  const { user } = useHealth();
  const [confirm, setConfirm] = useState("");
  const [msg, setMsg] = useState("");
  const del = async () => {
    try { await api("/me/data", { method: "DELETE", body: { confirm } }); setMsg("All your health data was deleted."); setConfirm(""); }
    catch (e) { setMsg(explain(e)); }
  };
  const revoke = async () => {
    try { const r = await api<{ revoked: number }>("/me/sessions/others", { method: "DELETE" }); setMsg(`Signed out ${r.revoked} other session(s).`); }
    catch (e) { setMsg(explain(e)); }
  };
  return (
    <Sub title="Privacy">
      <Group title="Your data" foot="A CSV of every reading stored for your account.">
        <a className="row" href="/api/v1/me/export.csv" download><span className="row-label tone-blue">Export My Data (CSV)</span></a>
      </Group>
      <Group title="Sessions"><Row onClick={revoke} label="Sign Out Other Devices" tone="blue" /></Group>
      <section className="card danger">
        <h3>Delete My Data</h3>
        <p className="muted small">Deletes all readings, workouts, goals, pins and marked awards for your account. Your login stays. Type <b>{user.username}</b> to confirm.</p>
        <input value={confirm} onChange={(e) => setConfirm(e.target.value)} aria-label="Confirm username" />
        <button className="pill red" disabled={confirm !== user.username} onClick={del}>Delete Everything</button>
      </section>
      {msg && <p className="muted">{msg}</p>}
    </Sub>
  );
}

export const SyncPage = () => <Sub title="iPhone Sync"><HaeSettings /><SyncSettings /></Sub>;
export const AppearancePage = () => { const { user, onUser } = useHealth(); return <Sub title="Appearance" wide><Appearance user={user} onUser={onUser} /></Sub>; };
export const PasswordPage = () => { const { user, onUser } = useHealth(); return <Sub title="Change Password"><ChangePassword user={user} onDone={onUser} /></Sub>; };
export const ImportPage = () => <Sub title="Import Health Data"><Import /></Sub>;
export const UsersPage = () => <Sub title="Users" wide><Admin /></Sub>;

export function useProfile(user: User | null) {
  const [p, setP] = useState<Profile | null>(null);
  useEffect(() => { if (user) api<{ profile: Profile }>("/profile").then((r) => setP(r.profile), () => setP(EMPTY_PROFILE)); }, [user?.id]); // eslint-disable-line react-hooks/exhaustive-deps
  return [p, setP] as const;
}
