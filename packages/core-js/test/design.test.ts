import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { initSync } from "../wasm/astra_wasm.js";
import { handle } from "../src/handler";
import { CoreClient, type WorkerLike } from "../src/client";
import type { CoreRequest, CoreResponse } from "../src/protocol";
import type { Design } from "../src/types";

initSync({ module: readFileSync(new URL("../wasm/astra_wasm_bg.wasm", import.meta.url)) });

/** Worker falso que atende pelo mesmo `handle` do worker real. */
function fakeWorker(): WorkerLike {
  const w: WorkerLike = {
    onmessage: null,
    onerror: null,
    postMessage(req: CoreRequest) {
      const res: CoreResponse = handle(req);
      queueMicrotask(() => w.onmessage?.({ data: res }));
    },
  };
  return w;
}

describe("projeto pelo worker", () => {
  const client = new CoreClient(fakeWorker());

  it("lista exemplos e materiais", async () => {
    const t = await client.templates();
    expect(t.map((x) => x.id)).toEqual(["pet", "competicao-500m"]);
    const m = await client.materials();
    expect(m.length).toBeGreaterThan(45);
    expect(m.every((x) => x.source.length > 0)).toBe(true);
  });

  it("verifica e voa o exemplo de competição", async () => {
    const d = (await client.templates()).find((x) => x.id === "competicao-500m")!.design;
    const v = await client.validate(d, true);
    expect(v.issues.filter((i) => i.severity === "error")).toEqual([]);
    expect(v.summary.apogee).toBeGreaterThan(450);
    const o = await client.simulate(d);
    expect(o.events.map((e) => e.kind)).toContain("ejection");
    expect(o.bodies[0]!.landing![1]).toBeLessThan(7);
  });

  it("coifa solta: aviso e separação no voo", async () => {
    const d = structuredClone((await client.templates()).find((x) => x.id === "pet")!.design) as Design;
    d.joints = d.joints.filter((j) => j.id !== "j-coifa");
    const v = await client.validate(d, false);
    expect(v.issues.some((i) => i.code === "notConnected" && i.parts.includes("coifa"))).toBe(true);
    const o = await client.simulate(d);
    expect(o.events.some((e) => e.kind === "jointBroken")).toBe(true);
    expect(o.bodies.length).toBe(2);
  });

  it("rejeita JSON inválido com mensagem", async () => {
    await expect(client.validate({ version: 1 } as unknown as Design, false)).rejects.toThrow(/projeto inválido/);
  });
});

describe("missão orbital pelo worker", () => {
  const client = new CoreClient(fakeWorker());
  it("Falcon 9 com 10 t chega a 200 km; motores listados", async () => {
    const o = await client.mission({
      vehicle: "falcon9",
      payload: 10_000,
      targetAltitude: 200_000,
      azimuthDeg: 90,
      latDeg: 28.5618,
      lonDeg: -80.5772,
      altitude: 3,
      utc: "2026-09-03T14:00:00Z",
      recoveryReserve: 0,
    });
    expect(o.reachedOrbit).toBe(true);
    expect(Math.abs(o.orbit!.perigeeAlt - 200_000)).toBeLessThan(15_000);
    expect(o.bodies.map((b) => b.name)).toContain("1º estágio");
    const e = await client.engines();
    expect(e.find((x) => x.name === "Merlin 1D")!.thrustSlKn).toBeCloseTo(845, 0);
  });
});
