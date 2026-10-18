import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite";
import { nodePolyfills } from "vite-plugin-node-polyfills";

export default defineConfig(({ command }) => ({
  plugins: [sveltekit(), nodePolyfills({
    include: ["crypto", "buffer", "process", "util", "stream", "vm"],
    globals: {
      Buffer: command === "build" ? true : "dev",
      global: command === "build" ? true : "dev",
      process: command === "build" ? true : "dev",
    },
  })],
  optimizeDeps: { include: ["@aurora-is-near/intents-swap-widget-standalone"] },
  server: { fs: { allow: ["./tests/fixtures"] } },
}));
