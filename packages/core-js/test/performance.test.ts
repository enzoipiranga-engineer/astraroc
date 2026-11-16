import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { designTemplates, initSync, simulateDesign, validateDesign } from "../wasm/astra_wasm.js";
import type { TemplateEntry } from "../src/types";

initSync({ module: readFileSync(new URL("../wasm/astra_wasm_bg.wasm", import.meta.url)) });

describe("desempenho", () => {
  it("foguete pequeno: voo completo em menos de 1 s (meta do projeto)", () => {
    const tpls = JSON.parse(designTemplates()) as TemplateEntry[];
    for (const id of ["pet", "competicao-500m", "competicao-carga"]) {
      const d = JSON.stringify(tpls.find((t) => t.id === id)!.design);
      simulateDesign(d); // aquece o JIT
      const t0 = performance.now();
      simulateDesign(d);
      const sim = performance.now() - t0;
      const t1 = performance.now();
      validateDesign(d, true);
      const val = performance.now() - t1;
      expect(sim, `${id}: voo em ${sim.toFixed(0)} ms`).toBeLessThan(1000);
      expect(val, `${id}: verificação com voo em ${val.toFixed(0)} ms`).toBeLessThan(1500);
    }
  });
});
