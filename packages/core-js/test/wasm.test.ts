import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import {
  initSync,
  atmosphere,
  coreVersion,
  flight3Constant,
  selfCheck,
  waterRocketFlight,
} from "../wasm/astra_wasm.js";
import { CoreClient, type WorkerLike } from "../src/client";
import type { CoreRequest } from "../src/protocol";

initSync({ module: readFileSync(new URL("../wasm/astra_wasm_bg.wasm", import.meta.url)) });

describe("núcleo WASM", () => {
  it("expõe a versão do workspace Cargo", () => {
    expect(coreVersion()).toMatch(/^\d+\.\d+\.\d+$/);
  });

  it("mantém dupla precisão dentro do runtime WASM", () => {
    // Vetor de ~6,4e6 m rodado e desrodado: erro de ida e volta abaixo de 10 nm.
    expect(selfCheck()).toBeLessThan(1e-8);
  });
});

describe("física no WASM", () => {
  it("atmosfera padrão ao nível do mar", () => {
    const [t, p, rho, a] = atmosphere(0, 0);
    expect(t).toBeCloseTo(288.15, 6);
    expect(p).toBeCloseTo(101325, 3);
    expect(rho).toBeCloseTo(1.225, 4);
    expect(a).toBeCloseTo(340.294, 2);
  });

  it("voo 3-DOF devolve os quatro eventos em ordem", () => {
    // Foguete de modelismo: 1,2 kg, 60 N por 1,6 s, trilho de 1 m a 85°.
    const rad = Math.PI / 180;
    const out = flight3Constant(
      new Float64Array([1.2, 60, 1.6, 0.03, 0, 0.004, -2.3 * rad, -44.4 * rad, 40, 1, 85 * rad, 0, 120, 0.05]),
    );
    const ev = Array.from(out.events);
    expect(ev.filter((_, i) => i % 4 === 0)).toEqual([0, 1, 2, 3]);
    const apogee = ev[2 * 4 + 2]!;
    expect(apogee).toBeGreaterThan(150);
    expect(apogee).toBeLessThan(1000);
    expect(out.samples.length % 8).toBe(0);
    expect(() => flight3Constant(new Float64Array(3))).toThrow(/14 parâmetros/);
  });
});

describe("foguete de água no WASM", () => {
  const rad = Math.PI / 180;
  const site = [-23.5 * rad, -46.6 * rad, 760, 88 * rad];

  it("garrafa PET de 2 L a 4 bar sobe algumas dezenas de metros", () => {
    const out = waterRocketFlight(new Float64Array([2e-3, 0.66e-3, 4e5, ...site]));
    const ev = Array.from(out.events);
    const apogee = ev[ev.findIndex((v, i) => i % 4 === 0 && v === 2) + 2]! - 760;
    expect(apogee).toBeGreaterThan(30);
    expect(apogee).toBeLessThan(80);
    expect(out.warnings).toEqual([]);
  });

  it("avisa quando a pressão passa do limite de segurança", () => {
    const out = waterRocketFlight(new Float64Array([2e-3, 0.66e-3, 9e5, ...site]));
    expect(out.warnings.join(" ")).toMatch(/limite de segurança/);
  });
});

describe("CoreClient", () => {
  it("casa respostas com pedidos pelo id", async () => {
    const worker: WorkerLike = {
      onmessage: null,
      onerror: null,
      postMessage(req: CoreRequest) {
        queueMicrotask(() =>
          worker.onmessage?.({
            data: {
              id: req.id,
              ok: true,
              kind: "info",
              info: { version: coreVersion(), selfCheckError: selfCheck() },
            },
          }),
        );
      },
    };
    const client = new CoreClient(worker);
    const [a, b] = await Promise.all([client.info(), client.info()]);
    expect(a.version).toBe(coreVersion());
    expect(b.selfCheckError).toBeLessThan(1e-8);
  });

  it("rejeita pedidos pendentes quando o worker falha", async () => {
    const worker: WorkerLike = { onmessage: null, onerror: null, postMessage() {} };
    const p = new CoreClient(worker).info();
    worker.onerror?.({ message: "boom" });
    await expect(p).rejects.toThrow("boom");
  });
});
