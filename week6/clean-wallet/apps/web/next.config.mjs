import { fileURLToPath } from "node:url";

/** @type {import('next').NextConfig} */
const nextConfig = {
  // @clean-wallet/core는 빌드 산출물이 아니라 TS 소스라 Next가 직접 트랜스파일한다.
  transpilePackages: ["@clean-wallet/core"],
  // 모노레포 루트를 명시 — SSR Lambda 파일 트레이싱이 workspace node_modules와
  // @clean-wallet/core를 올바르게 포함하도록 (apps/web → ../../ = clean-wallet).
  outputFileTracingRoot: fileURLToPath(new URL("../../", import.meta.url)),
};

export default nextConfig;
