// Shared iOS-style chrome: large titles, round buttons, sheets, grouped rows.
import { Link, navigate, usePath } from "../router";
import { useEffect, useRef, useState } from "react";
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

/** Square icon box. Glyphs are centered in the viewBox so a centered button stays centered. */
function Glyph({ children }: { children: React.ReactNode }) {
  return <svg width="22" height="22" viewBox="0 0 24 24" aria-hidden focusable="false">{children}</svg>;
}
export const BackIcon = () => <Glyph><path d="M15 6 9 12l6 6" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" /></Glyph>;
export const XIcon = () => <Glyph><path d="M7 7l10 10M17 7 7 17" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" /></Glyph>;
export const CheckIcon = () => <Glyph><path d="M6 12.5 10.2 16.5 18 8" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" /></Glyph>;
export const CalIcon = () => (
  <Glyph>
    <rect x="4.5" y="6" width="15" height="14" rx="2.5" fill="none" stroke="currentColor" strokeWidth="1.8" />
    <path d="M4.5 10.5h15M8 3.8v4.2M16 3.8v4.2" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
  </Glyph>
);

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

export function Sheet({ title, onClose, children, left, right }: { title: string; onClose: () => void; children: React.ReactNode; left?: "x" | "back"; right?: React.ReactNode }) {
  const startY = useRef<number | null>(null);
  const [dy, setDy] = useState(0);
  const onTouchStart = (e: React.TouchEvent) => {
    const body = (e.currentTarget as HTMLElement).querySelector(".sheet-body") as HTMLElement | null;
    if (body && body.scrollTop > 0) { startY.current = null; return; }
    startY.current = e.touches[0].clientY;
    setDy(0);
  };
  const onTouchMove = (e: React.TouchEvent) => {
    if (startY.current == null) return;
    const d = Math.max(0, e.touches[0].clientY - startY.current);
    setDy(d);
  };
  const onTouchEnd = () => {
    if (dy > 110) onClose();
    startY.current = null;
    setDy(0);
  };
  return (
    <div className="sheet-backdrop" onClick={onClose}>
      <div
        className="sheet"
        role="dialog"
        aria-label={title}
        onClick={(e) => e.stopPropagation()}
        onTouchStart={onTouchStart}
        onTouchMove={onTouchMove}
        onTouchEnd={onTouchEnd}
        style={dy ? { transform: `translateY(${dy}px)`, transition: "none" } : undefined}
      >
        <div className="sheet-head">
          <RoundBtn label={left === "back" ? "Back" : "Close"} onClick={onClose}>{left === "back" ? <BackIcon /> : <XIcon />}</RoundBtn>
          <b>{title}</b>
          <div className="nb-right">{right ?? <span style={{ width: 36 }} />}</div>
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
