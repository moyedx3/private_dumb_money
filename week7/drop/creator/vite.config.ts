import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

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
