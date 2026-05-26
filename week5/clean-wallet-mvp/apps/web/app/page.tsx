import Link from "next/link";

export default function Home() {
  return (
    <main>
      <h1>Clean Wallet MVP</h1>
      <p>Attested Zcash testnet scanner for private off-ramp screening.</p>
      <ul>
        <li><Link href="/prover">User (prover)</Link></li>
        <li><Link href="/verifier">Exchange (verifier)</Link></li>
      </ul>
    </main>
  );
}
