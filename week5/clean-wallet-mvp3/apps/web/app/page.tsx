import Link from "next/link";

export default function Home() {
  return (
    <main>
      <section className="hero">
        <p className="eyebrow">Zcash private off-ramp screening</p>
        <h1>Screen a private wallet. Share only the decision.</h1>
        <p>
          Compare a clean wallet with a wallet that paid a sanctioned recipient.
          The measured scanner runs inside Phala Cloud; the exchange receives an
          attested PASS or FAIL artifact without receiving wallet records.
        </p>
        <div className="heroActions">
          <Link className="primaryLink" href="/prover">Start guided wallet demo</Link>
          <Link className="secondaryLink" href="/verifier">See exchange decision</Link>
        </div>
      </section>

      <section className="workflowGrid" aria-label="Demo workflow">
        <article className="panel workflowCard">
          <span className="step">01</span>
          <p className="eyebrow">Choose a scenario</p>
          <h2>Clean or sanctioned?</h2>
          <p>Switch between the two wallet stories with one click and inspect the expected private scan.</p>
        </article>
        <article className="panel workflowCard">
          <span className="step">02</span>
          <p className="eyebrow">Private screening</p>
          <h2>Run inside the TEE</h2>
          <p>The scanner checks outgoing recipients against the sanctioned set without exposing wallet history.</p>
        </article>
        <article className="panel workflowCard">
          <span className="step">03</span>
          <p className="eyebrow">Exchange verification</p>
          <h2>Approve or reject deposit</h2>
          <p>The exchange validates the artifact and sees exactly why the deposit is accepted or rejected.</p>
        </article>
      </section>
    </main>
  );
}
