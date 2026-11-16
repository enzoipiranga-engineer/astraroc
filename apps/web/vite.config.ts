import react from "@vitejs/plugin-react";
import { readdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { defineConfig, type Plugin } from "vite";
import { viteStaticCopy } from "vite-plugin-static-copy";

const cesium = "node_modules/cesium/Build/Cesium";

/** Lista os arquivos do app (sem o Cesium) para o service worker baixar na instalação. */
function precache(): Plugin {
  let out = "dist";
  return {
    name: "astra-precache",
    apply: "build",
    configResolved(c) {
      out = resolve(c.root, c.build.outDir);
    },
    closeBundle() {
      const files = ["./", ...readdirSync(resolve(out, "assets")).filter((f) => !f.endsWith(".map")).map((f) => `./assets/${f}`), "./manifest.webmanifest", "./icon-512.png"];
      writeFileSync(resolve(out, "precache.json"), JSON.stringify(files));
    },
  };
}

export default defineConfig({
  plugins: [
    react(),
    precache(),
    // Workers, texturas e estilos do Cesium servidos em /cesium (ver src/cesium-setup.ts).
    viteStaticCopy({
      targets: ["Workers", "ThirdParty", "Assets", "Widgets"].map((d) => ({ src: `${cesium}/${d}`, dest: "cesium", rename: { stripBase: 4 } })),
    }),
  ],
  base: "./",
  worker: { format: "es" },
  build: { target: "es2022", chunkSizeWarningLimit: 6000 },
});
