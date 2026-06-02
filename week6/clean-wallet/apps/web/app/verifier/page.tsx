import { VerifierForm } from "./verifier-form";

export default function VerifierPage() {
  return (
    <section className="stack">
      <h1>Exchange Verifier</h1>
      <p className="lead">
        거래소가 받은 artifact를 검증합니다. 6개 항목(measurement·서명·정책·입금·구간·nonce)을
        모두 통과해야 결과를 신뢰합니다. JSON을 직접 수정해 변조 탐지도 시험해 보세요.
      </p>
      <VerifierForm />
    </section>
  );
}
