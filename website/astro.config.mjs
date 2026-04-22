import { defineConfig } from "astro/config";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  redirects: {
    "/docs": "/",
    "/docs/what-is-caldir": "/",
    "/docs/getting-started": "/quickstart",
    "/docs/commands": "/commands",
    "/docs/providers": "/providers",
    "/docs/configuration": "/configuration",
  },
  vite: {
    plugins: [tailwindcss()],
  },
  markdown: {
    shikiConfig: {
      theme: "min-light",
    },
  },
});
