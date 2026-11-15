import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { initSync, coreVersion, selfCheck } from "../wasm/astra_wasm.js";
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
