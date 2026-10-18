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
  optimizeDeps: {
    include: ["pay3flow-near-sdk"],
    // Respect SDK pure annotations so unused provider UI/icons do not enter
    // the browser's development bundle through the service hooks.
    esbuildOptions: { ignoreAnnotations: false },
  },
  server: { fs: { allow: ["./tests/fixtures"] } },
}));
