import { defineConfig } from "vitest/config";

const sodiumWrapper = new URL(
  "./node_modules/libsodium-wrappers/dist/modules/libsodium-wrappers.js",
  import.meta.url
).pathname;

export default defineConfig({
  resolve: {
    alias: {
      "libsodium-wrappers": sodiumWrapper
    }
  },
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"]
  }
});
