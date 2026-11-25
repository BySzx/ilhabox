import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import solid from "vite-plugin-solid";
import { readFileSync } from "node:fs";

// Two pages: box.html is what the phone shows, index.html is the settings window on the PC.
export default defineConfig({
  plugins: [solid()],
  define: { __VERSAO__: JSON.stringify(JSON.parse(readFileSync(new URL("package.json", import.meta.url), "utf8")).version) },
  clearScreen: false,
  server: { port: 1430, strictPort: true, watch: { ignored: ["**/src-tauri/**", "**/android/**"] } },
  build: {
    target: "es2019",
    cssMinify: true,
    reportCompressedSize: false,
    rollupOptions: { input: { index: fileURLToPath(new URL("index.html", import.meta.url)), box: fileURLToPath(new URL("box.html", import.meta.url)) } },
  },
});
