import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Same alias C uses: point at the prebuilt module so the WASM loads cleanly under Vite.
const sodiumWrapper = new URL(
  "./node_modules/libsodium-wrappers/dist/modules/libsodium-wrappers.js",
  import.meta.url
).pathname;

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "libsodium-wrappers": sodiumWrapper
    }
  }
});
