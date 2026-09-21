import adapter from "@sveltejs/adapter-node";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

export default {
  preprocess: vitePreprocess(),
  compilerOptions: {
    // The stylesheet is intentionally copied verbatim from the Next.js source.
    // Keep dormant selectors so future source-side UI states retain visual parity.
    warningFilter: (warning) => warning.code !== "css_unused_selector",
  },
  kit: {
    adapter: adapter(),
  },
};
