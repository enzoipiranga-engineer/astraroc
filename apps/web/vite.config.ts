import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import { viteStaticCopy } from "vite-plugin-static-copy";

const cesium = "node_modules/cesium/Build/Cesium";

export default defineConfig({
  plugins: [
    react(),
    // Workers, texturas e estilos do Cesium servidos em /cesium (ver src/cesium-setup.ts).
    viteStaticCopy({
      targets: ["Workers", "ThirdParty", "Assets", "Widgets"].map((d) => ({ src: `${cesium}/${d}`, dest: "cesium", rename: { stripBase: 4 } })),
    }),
  ],
  worker: { format: "es" },
  build: { target: "es2022", chunkSizeWarningLimit: 6000 },
});
