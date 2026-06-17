import "./globals.css";
import Link from "next/link";

export const metadata = {
  title: "Clean Wallet MVP3",
  description: "Attested Zcash private off-ramp screening demo",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>
        <header className="siteHeader">
          <Link className="brand" href="/">Clean Wallet MVP3</Link>
          <nav>
            <Link href="/prover">User scanner</Link>
            <Link href="/verifier">Exchange verifier</Link>
          </nav>
        </header>
        {children}
      </body>
    </html>
  );
}
