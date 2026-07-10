import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { nodePolyfills } from "vite-plugin-node-polyfills";

const sodiumWrapper = new URL(
  "./node_modules/libsodium-wrappers/dist/modules/libsodium-wrappers.js",
  import.meta.url
).pathname;

export default defineConfig({
  plugins: [
    react(),
    nodePolyfills({
      include: ["buffer", "crypto", "stream", "util"],
      globals: {
        Buffer: true,
        global: true,
        process: true
      }
    })
  ],
  resolve: {
    alias: {
      "libsodium-wrappers": sodiumWrapper
    }
  }
});
