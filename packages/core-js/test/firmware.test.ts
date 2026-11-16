import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { designTemplates, electronicsLibrary, initSync, simulateDesign, validateDesign } from "../wasm/astra_wasm.js";
import { Bmp280, Mpu6050, nmeaGga } from "../src/avr/devices";
import { fly } from "../src/avr/fly";
import { parseHex } from "../src/avr/hex";
import { arduinoPin } from "../src/avr/machine";
import type { Design, ElecComponent, SimOutput, TemplateEntry, Validation } from "../src/types";

initSync({ module: readFileSync(new URL("../wasm/astra_wasm_bg.wasm", import.meta.url)) });

const avionics = () => (JSON.parse(designTemplates()) as TemplateEntry[]).find((t) => t.id === "competicao-avionica")!.design;
const lib = () => JSON.parse(electronicsLibrary()) as ElecComponent[];
const sim = (d: Design) => JSON.parse(simulateDesign(JSON.stringify(d))) as SimOutput;
const val = (d: Design) => JSON.parse(validateDesign(JSON.stringify(d), false)) as Validation;
const apogee = (o: SimOutput) => o.events.find((e) => e.kind === "apogee")!;

describe("dispositivos virtuais", () => {
  it("BMP280: o valor cru reconstrói a pressão e a temperatura pelas fórmulas do datasheet", () => {
    // Exemplo do datasheet (§8.1, tabela 16): adc_T = 519888 → 25,08 °C; adc_P = 415148 → 100653 Pa.
    const t = Bmp280.compT(519888);
    expect(t.t100).toBe(2508);
    expect(Bmp280.compP(415148, t.tFine) / 256).toBeCloseTo(100653.27, 0);
    for (const [p, c] of [[101325, 15], [54000, -20], [95800, 12]] as const) {
      const rt = Bmp280.rawTemperature(c);
      const rp = Bmp280.rawPressure(p, rt.tFine);
      expect(Math.abs(Bmp280.compP(rp, rt.tFine) / 256 - p)).toBeLessThan(0.5);
    }
  });

  it("MPU-6050: escala e saturação", () => {
    const m = new Mpu6050(0x68);
    m.reading = { pressure: 1e5, temperature: 300, accel: [100 * 9.80665, 0, -9.80665], gyro: [1, 0, 0] };
    for (const [r, v] of [[0x6b, 0], [0x1c, 0x18]]) {
      m.start();
      m.write(r!);
      m.write(v!);
    }
    m.start();
    m.write(0x75);
    expect(m.read()).toBe(0x68);
    m.start();
    m.write(0x3b);
    const word = () => {
      const v = (m.read() << 8) | m.read();
      return v & 0x8000 ? v - 0x10000 : v;
    };
    expect(word()).toBe(32767); // 100 g satura em ±16 g
    expect(word()).toBe(0);
    expect(word()).toBe(-2048); // −1 g a ±16 g: 32768/16
  });

  it("NMEA com soma de verificação e pinos do Arduino", () => {
    const s = nmeaGga(43200, -22.18, -49.38, 470, true, 8);
    expect(s).toMatch(/^\$GPGGA,120000\.00,2210\.8000,S,04922\.8000,W,1,08/);
    const body = s.slice(1, s.indexOf("*"));
    let x = 0;
    for (const c of body) x ^= c.charCodeAt(0);
    expect(s.slice(s.indexOf("*") + 1, s.indexOf("*") + 3)).toBe(x.toString(16).toUpperCase().padStart(2, "0"));
    expect(arduinoPin("D3")).toEqual({ port: "D", bit: 3 });
    expect(arduinoPin("D13")).toEqual({ port: "B", bit: 5 });
    expect(arduinoPin("A4")).toEqual({ port: "C", bit: 4 });
  });

  it("HEX: rejeita soma de verificação errada", () => {
    expect(() => parseHex(":0100000001FF\n:00000001FF")).toThrow(/soma/);
    expect(parseHex(":0100000001FE\n:00000001FF")[0]).toBe(1);
  });
});

describe("firmware no laço", () => {
  it("o firmware de exemplo detecta o apogeu e abre o paraquedas", { timeout: 120_000 }, () => {
    const d = avionics();
    const r = fly(d, val(d), lib(), sim);
    const lines = r.serial.map((s) => s.line);
    expect(lines).toContain("BMP280 OK");
    expect(lines.some((l) => l.startsWith("DECOLAGEM"))).toBe(true);
    expect(r.pyroEvents).toHaveLength(1);
    const ballistic = sim(d);
    const ta = apogee(ballistic).t;
    // Dispara depois do apogeu verdadeiro, com o atraso da detecção (3 m abaixo, 5 leituras).
    expect(r.pyroEvents[0]!.t).toBeGreaterThan(ta);
    expect(r.pyroEvents[0]!.t).toBeLessThan(ta + 3);
    expect(r.output.events.some((e) => e.kind === "ejection")).toBe(true);
    const landing = (o: SimOutput) => Math.max(...o.bodies.map((b) => b.landing?.[1] ?? 0));
    expect(landing(r.output)).toBeLessThan(0.6 * landing(ballistic));
    // A altitude que o firmware registrou é a da fórmula hipsométrica de atmosfera padrão
    // aplicada à pressão verdadeira no apogeu (±1,5 m de ruído e quantização). Ela difere da
    // altura geométrica em ~1 %: a fórmula supõe 15 °C na pressão de referência, mas o
    // solo a 470 m está a ~12 °C — erro real dos altímetros barométricos.
    const apoLine = lines.find((l) => l.startsWith("APOGEU"))!;
    const fw = Number(apoLine.split(",")[1]) / 10;
    const rows = ballistic.sensors!;
    const pApo = Math.min(...rows.map((x) => x[1]!));
    const expected = 44330 * (1 - (pApo / rows[0]![1]!) ** 0.190295);
    expect(Math.abs(fw - expected)).toBeLessThan(1.5);
    expect(Math.abs(fw - apogee(ballistic).alt) / apogee(ballistic).alt).toBeLessThan(0.02);
    expect(r.pins.some((p) => p.pin === "D3" && p.high)).toBe(true);
    expect(r.trace.length).toBeGreaterThan(50);
  });

  it("bateria 9 V: o disparo derruba a tensão e a placa reinicia", { timeout: 120_000 }, () => {
    const d = avionics();
    d.electronics.components.find((c) => c.id === "lipo")!.lib = "bateria-9v";
    const r = fly(d, val(d), lib(), sim);
    expect(r.resets).toBe(1);
    expect(r.notes.join(" ")).toMatch(/brownout/);
  });

  it("baia sem respiro: o barômetro não vê a subida e o paraquedas não abre", { timeout: 120_000 }, () => {
    const d = avionics();
    for (const p of d.parts) if (p.kind.type === "bodyTube") p.kind.ventHoles = 0;
    const v = val(d);
    expect(v.issues.some((i) => i.code === "noStaticPort")).toBe(true);
    const r = fly(d, v, lib(), sim);
    expect(r.pyroEvents).toHaveLength(0);
    expect(r.notes.join(" ")).toMatch(/respiro/);
  });
});
