/// <reference lib="webworker" />
/**
 * Corpo do Web Worker do núcleo. O app cria um arquivo de worker que apenas
 * importa este módulo, para o bundler resolver o .wasm como asset.
 */
import init, { coreVersion, selfCheck } from "../wasm/astra_wasm.js";
import type { CoreRequest, CoreResponse } from "./protocol";

const ready = init();
const scope = self as unknown as DedicatedWorkerGlobalScope;

scope.onmessage = async (ev: MessageEvent<CoreRequest>) => {
  const req = ev.data;
  let res: CoreResponse;
  try {
    await ready;
    switch (req.kind) {
      case "info":
        res = {
          id: req.id,
          ok: true,
          kind: "info",
          info: { version: coreVersion(), selfCheckError: selfCheck() },
        };
    }
  } catch (e) {
    res = { id: req.id, ok: false, error: e instanceof Error ? e.message : String(e) };
  }
  scope.postMessage(res);
};
