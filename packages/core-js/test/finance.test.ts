import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { designTemplates, electronicsLibrary, initSync, validateDesign } from "../wasm/astra_wasm.js";
import { budget, buildBom, financeOf, paretoFront, ratesFromFrankfurter, setEntry, setFinance, subsystemOfPart, toBrl } from "../src/finance";
import type { Design, ElecComponent, TemplateEntry, Validation } from "../src/types";

initSync({ module: readFileSync(new URL("../wasm/astra_wasm_bg.wasm", import.meta.url)) });
const tpl = (id: string) => structuredClone((JSON.parse(designTemplates()) as TemplateEntry[]).find((t) => t.id === id)!.design);
const lib = JSON.parse(electronicsLibrary()) as ElecComponent[];
const masses = (d: Design) => (JSON.parse(validateDesign(JSON.stringify(d), false)) as Validation).summary.partMasses!;

describe("lista de materiais", () => {
  it("gera uma linha por peça, componente e motor, com subsistemas", () => {
    const d = tpl("competicao-avionica");
    const bom = buildBom(d, lib, masses(d));
    expect(bom.filter((l) => l.key.startsWith("part:"))).toHaveLength(d.parts.length);
    expect(bom.filter((l) => l.key.startsWith("elec:"))).toHaveLength(d.electronics.components.length);
    expect(bom.find((l) => l.key === "motor")?.consumable).toBe(true);
    expect(bom.find((l) => l.key === "elec:em1")?.subsystem).toBe("recuperacao");
    expect(bom.find((l) => l.key === "elec:em1")?.consumable).toBe(true);
    expect(subsystemOfPart(d, "paraquedas")).toBe("recuperacao");
    expect(subsystemOfPart(d, "altimetro")).toBe("avionica");
    expect(subsystemOfPart(d, "aletas")).toBe("estrutura");
    expect(bom.every((l) => l.missing)).toBe(true); // nada tem preço no exemplo
  });

  it("converte moeda, soma frete e impostos e estima pelo preço do material", () => {
    let d = tpl("competicao-500m");
    d = setFinance(d, { rates: { USD: 5, EUR: 6, date: "2026-11-13", source: "teste" }, materialPrices: { "g10-fr4": 200 } });
    d = setEntry(d, "motor", { price: 20, currency: "USD", qty: 2, shipping: 10, taxPct: 60 });
    const m = masses(d);
    const bom = buildBom(d, lib, m);
    // (20·2 + 10)·1,6·5 = 400.
    expect(bom.find((l) => l.key === "motor")!.totalBrl).toBeCloseTo(400, 9);
    const finIdx = d.parts.findIndex((p) => p.id === "aletas");
    const fins = bom.find((l) => l.key === "part:aletas")!;
    expect(fins.estimated).toBe(true);
    expect(fins.totalBrl).toBeCloseTo(m[finIdx]! * 200, 9);
    expect(toBrl(10, "EUR", null)).toBeNaN();
  });

  it("orçamento, teto, receitas, saldo e custo por voo", () => {
    let d = tpl("competicao-500m");
    d.parts.forEach((p) => (p.cost = 10));
    d = setFinance(d, { cap: 100, caps: { estrutura: 50 }, flightsPlanned: 4, revenues: [{ id: "r1", source: "Empresa X", kind: "patrocinio", amount: 300, currency: "BRL", status: "recebido" }, { id: "r2", source: "Edital", kind: "edital", amount: 500, currency: "BRL", status: "prometido" }] });
    d = setEntry(d, "motor", { price: 80, consumable: true, actual: 95, status: "recebido", critical: true });
    const bom = buildBom(d, lib, masses(d));
    const b = budget(d, bom);
    const parts = d.parts.length * 10;
    expect(b.planned).toBeCloseTo(parts + 80, 9);
    expect(b.actual).toBeCloseTo(95, 9);
    expect(b.alerts.some((a) => a.includes("teto total"))).toBe(true);
    expect(b.alerts.some((a) => a.startsWith("Estrutura"))).toBe(true);
    expect(b.alerts.some((a) => a.includes("10 %"))).toBe(true);
    expect(b.revenueReceived).toBe(300);
    expect(b.balance).toBeCloseTo(300 - 95, 9);
    expect(b.projected).toBeCloseTo(800 - (parts + 80), 9);
    expect(b.costPerFlight).toBeCloseTo(80 + parts / 4, 9);
    expect(financeOf(d).entries).toHaveLength(1);
  });

  it("câmbio da Frankfurter e fronteira de Pareto", () => {
    const r = ratesFromFrankfurter({ amount: 1, base: "USD", date: "2026-11-13", rates: { BRL: 5.5, EUR: 0.92 } });
    expect(r.USD).toBe(5.5);
    expect(r.EUR).toBeCloseTo(5.5 / 0.92, 12);
    expect(() => ratesFromFrankfurter({ base: "EUR", rates: {} })).toThrow();
    const pts = [
      { id: "a", cost: 100, apogee: 500, safety: 2 },
      { id: "b", cost: 120, apogee: 480, safety: 1.5 }, // dominado por a
      { id: "c", cost: 80, apogee: 450, safety: 2 },
      { id: "d", cost: 150, apogee: 600, safety: 1.2 },
    ];
    expect(paretoFront(pts).map((p) => p.id).sort()).toEqual(["a", "c", "d"]);
  });
});
