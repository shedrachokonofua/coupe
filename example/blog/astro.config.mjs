import { defineConfig } from "astro/config";
import svelte from "@astrojs/svelte";
import node from "@astrojs/node";

// https://astro.build/config
export default defineConfig({
  output: "server",
  adapter: node({
    mode: "middleware",
  }),
  integrations: [svelte()],
  markdown: {
    shikiConfig: {
      theme: "github-light",
      wrap: true,
    },
  },
});
