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
