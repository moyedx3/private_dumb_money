import { ProverForm } from "./prover-form";

export default function ProverPage() {
  return (
    <section className="stack">
      <h1>Prover / Scanner</h1>
      <p className="lead">
        viewing scope를 골라 스캔하면 attested scanner가 블록 구간 전체를 검사해 screening
        artifact를 만듭니다. raw 거래내역(수취인·금액)은 artifact에 담기지 않습니다.
      </p>
      <ProverForm />
    </section>
  );
}
