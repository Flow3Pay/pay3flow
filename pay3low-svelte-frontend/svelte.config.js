import adapter from "@sveltejs/adapter-node";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

export default {
  preprocess: vitePreprocess(),
  compilerOptions: {
    // Keep dormant selectors for UI states that are not always rendered.
    warningFilter: (warning) => warning.code !== "css_unused_selector",
  },
  kit: {
    adapter: adapter(),
  },
};
