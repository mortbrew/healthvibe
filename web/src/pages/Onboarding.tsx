import { useState } from "react";
import { api, User } from "../api";
import { FirstData, HaeConnect, HaeSteps, SponsorNote } from "../components/Hae";
import { Link, navigate } from "../router";

/** First-login setup: token → Health Auto Export steps → optional export.zip. Skippable. */
export default function Onboarding({ user, onUser }: { user: User; onUser: (u: User) => void }) {
  const [step, setStep] = useState(0);
  const finish = async () => {
    await api("/me/onboarding", { method: "PUT", body: { done: true } }).catch(() => {});
    onUser({ ...user, onboarding_done: true });
    navigate("/");
  };
  const steps = ["Welcome", "Connect your iPhone", "Past data", "Done"];
  return (
    <div className="help onboarding">
      <header className="pagehead">
        <p className="eyebrow">Step {step + 1} of {steps.length} · {steps[step]}</p>
        <h1>{step === 0 ? `Welcome to healthvibe` : steps[step]}</h1>
        <button className="ghost small" onClick={finish}>Skip for now</button>
      </header>
      {step === 0 && (
        <section className="card">
          <p className="lead">
            healthvibe shows your Apple Health data on your own server. Your iPhone sends it with the
            <b> Health Auto Export</b> app. Setup takes about five minutes.
          </p>
          <p className="muted">You can skip this and come back any time from Settings or Help.</p>
          <SponsorNote />
          <button onClick={() => setStep(1)}>Start</button>
        </section>
      )}
      {step === 1 && (
        <>
          <HaeSteps connect={<HaeConnect />} />
          <div className="row">
            <button className="ghost" onClick={() => setStep(0)}>Back</button>
            <button onClick={() => setStep(2)}>Next</button>
          </div>
        </>
      )}
      {step === 2 && (
        <section className="card">
          <p>
            Optional: load your full history once. In the Health app tap your picture → <b>Export All Health Data</b>,
            then upload the <code>export.zip</code> on the <Link to="/settings/import">Import</Link> page. Readings already sent by
            Health Auto Export may appear twice for the overlapping days.
          </p>
          <div className="row">
            <button className="ghost" onClick={() => setStep(1)}>Back</button>
            <Link to="/settings/import" className="button">Import export.zip now</Link>
            <button onClick={() => setStep(3)}>Skip import</button>
          </div>
        </section>
      )}
      {step === 3 && (
        <section className="card">
          <FirstData />
          <p>That's it. New data arrives whenever Health Auto Export syncs.</p>
          <button onClick={finish}>Go to my dashboard</button>
        </section>
      )}
    </div>
  );
}
