// Shared iOS-style chrome: large titles, round buttons, sheets, grouped rows.
import { Link, navigate, usePath } from "../router";
import { useEffect, useState } from "react";
import { AvatarSvg } from "./avatar";
import { useHealth } from "./ctx";
import { Chevron } from "./meta";

export function AvatarButton({ size = 36 }: { size?: number }) {
  const { avatar, name } = useHealth();
  return (
    <button className="avatarbtn" aria-label={`Account for ${name}`} onClick={() => navigate("/account")}>
      <AvatarSvg cfg={avatar} size={size} />
    </button>
  );
}

export function LargeTitle({ title, eyebrow, avatar = true, right }: { title: string; eyebrow?: string; avatar?: boolean; right?: React.ReactNode }) {
  return (
    <header className="largetitle">
      <div>
        {eyebrow && <p className="eyebrow">{eyebrow}</p>}
        <h1>{title}</h1>
      </div>
      <div className="lt-right">{right}{avatar && <AvatarButton />}</div>
    </header>
  );
}

export function RoundBtn({ label, onClick, children, disabled }: { label: string; onClick?: () => void; children: React.ReactNode; disabled?: boolean }) {
  return <button className="roundbtn" aria-label={label} title={label} onClick={onClick} disabled={disabled}>{children}</button>;
}

export const BackIcon = () => <svg width="11" height="18" viewBox="0 0 11 18" aria-hidden><path d="M9 2 2 9l7 7" fill="none" stroke="currentColor" strokeWidth="2.6" strokeLinecap="round" strokeLinejoin="round" /></svg>;
export const XIcon = () => <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden><path d="M2 2l10 10M12 2 2 12" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" /></svg>;
export const CalIcon = () => <svg width="18" height="18" viewBox="0 0 18 18" aria-hidden><rect x="2" y="3.5" width="14" height="12.5" rx="3" fill="none" stroke="currentColor" strokeWidth="1.8" /><path d="M2 7.5h14M6 2v3M12 2v3" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" /></svg>;

export function back(fallback: string) {
  if (history.length > 1 && document.referrer !== "" ) history.back();
  else navigate(fallback);
}

/** Header with a round back button, optional centered title and right-side slot. */
export function NavBar({ to, title, sub, right }: { to: string; title?: string; sub?: string; right?: React.ReactNode }) {
  return (
    <div className="navbar">
      <RoundBtn label="Back" onClick={() => navigate(to)}><BackIcon /></RoundBtn>
      <div className="nb-title">{title && <b>{title}</b>}{sub && <span className="muted">{sub}</span>}</div>
      <div className="nb-right">{right}</div>
    </div>
  );
}

export function Sheet({ title, onClose, children, left }: { title: string; onClose: () => void; children: React.ReactNode; left?: "x" | "back" }) {
  return (
    <div className="sheet-backdrop" onClick={onClose}>
      <div className="sheet" role="dialog" aria-label={title} onClick={(e) => e.stopPropagation()}>
        <div className="sheet-head">
          <RoundBtn label={left === "back" ? "Back" : "Close"} onClick={onClose}>{left === "back" ? <BackIcon /> : <XIcon />}</RoundBtn>
          <b>{title}</b>
          <span style={{ width: 36 }} />
        </div>
        <div className="sheet-body">{children}</div>
      </div>
    </div>
  );
}

export function Group({ children, title, foot }: { children: React.ReactNode; title?: string; foot?: React.ReactNode }) {
  return (
    <div className="group-wrap">
      {title && <p className="group-title">{title}</p>}
      <div className="group">{children}</div>
      {foot && <p className="group-foot">{foot}</p>}
    </div>
  );
}

export function Row({ to, onClick, icon, label, value, tone, chevron = true }: { to?: string; onClick?: () => void; icon?: React.ReactNode; label: React.ReactNode; value?: React.ReactNode; tone?: "green" | "red" | "blue"; chevron?: boolean }) {
  const inner = (
    <>
      {icon && <span className="row-icon">{icon}</span>}
      <span className={tone ? `row-label tone-${tone}` : "row-label"}>{label}</span>
      {value !== undefined && <span className="row-value">{value}</span>}
      {chevron && !tone && <Chevron />}
    </>
  );
  if (to) return <RowLink to={to}>{inner}</RowLink>;
  return <button className="row" onClick={onClick}>{inner}</button>;
}

function RowLink({ to, children }: { to: string; children: React.ReactNode }) {
  const path = usePath();
  return <Link to={to} className={path === to ? "row active" : "row"} aria-current={path === to ? "page" : undefined}>{children}</Link>;
}

/** Layout breakpoints (keep in sync with styles.css): tablet >= 768, desktop >= 1200. */
export const BP = { tablet: 768, desktop: 1200 };

export function useMinWidth(px: number) {
  const q = `(min-width: ${px}px)`;
  const [on, setOn] = useState(() => typeof window !== "undefined" && window.matchMedia(q).matches);
  useEffect(() => {
    const m = window.matchMedia(q);
    const f = () => setOn(m.matches);
    f();
    m.addEventListener("change", f);
    return () => m.removeEventListener("change", f);
  }, [q]);
  return on;
}
