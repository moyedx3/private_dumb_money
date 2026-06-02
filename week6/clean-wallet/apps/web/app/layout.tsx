import type { Metadata } from "next";
import type { ReactNode } from "react";
import Link from "next/link";
import "./globals.css";

export const metadata: Metadata = {
  title: "Zcash Private Off-Ramp Screening",
  description: "Attested scanner MVP 데모",
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="ko">
      <body>
        <header className="site-header">
          <Link href="/" className="brand">
            Zcash Off-Ramp Screening
          </Link>
          <nav>
            <Link href="/prover">Prover / Scanner</Link>
            <Link href="/verifier">Exchange Verifier</Link>
            <Link href="/results">결과 (DB)</Link>
          </nav>
        </header>
        <main className="container">{children}</main>
        <footer className="site-footer">
          MVP 데모 · mock 체인 · 시뮬레이션 attestation — 실제 컴플라이언스 도구가 아닙니다.
        </footer>
      </body>
    </html>
  );
}
