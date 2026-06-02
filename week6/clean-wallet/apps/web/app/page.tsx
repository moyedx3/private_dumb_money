import Link from "next/link";

export default function HomePage() {
  return (
    <section className="stack">
      <h1>Zcash Private Off-Ramp Screening</h1>
      <p className="lead">
        Zcash 사용자가 전체 거래내역을 공개하지 않고도, 자신의 지갑이 제재 대상 주소와
        직접 거래한 적이 없음을 거래소에 검증 가능하게 증명하는 데모입니다.
      </p>

      <div className="cards">
        <Link href="/prover" className="card">
          <h2>Prover / Scanner →</h2>
          <p className="muted">viewing scope를 스캔해 screening artifact를 생성합니다.</p>
        </Link>
        <Link href="/verifier" className="card">
          <h2>Exchange Verifier →</h2>
          <p className="muted">artifact를 검증해 결과를 신뢰할 수 있는지 판정합니다.</p>
        </Link>
        <Link href="/results" className="card">
          <h2>결과 (DB) →</h2>
          <p className="muted">
            배포된 TEE 스캐너가 만든 artifact를 DB에서 조회·재검증합니다.
          </p>
        </Link>
      </div>

      <div className="note">
        <strong>이것이 증명하지 않는 것:</strong> 사용자의 모든 지갑이 깨끗하다거나, ZEC의
        전체 출처가 깨끗하다거나, 완전한 OFAC/AML 컴플라이언스를 의미하지 않습니다. 좁은
        스크리닝 신호일 뿐입니다. (mock 체인 · 시뮬레이션 attestation 기반 데모)
      </div>
    </section>
  );
}
