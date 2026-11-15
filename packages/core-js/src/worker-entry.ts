/// <reference lib="webworker" />
/**
 * Corpo do Web Worker do núcleo. O app cria um arquivo de worker que apenas
 * importa este módulo, para o bundler resolver o .wasm como asset.
 */
import init from "../wasm/astra_wasm.js";
import { handle } from "./handler";
import type { CoreRequest } from "./protocol";

const ready = init();
const scope = self as unknown as DedicatedWorkerGlobalScope;

scope.onmessage = async (ev: MessageEvent<CoreRequest>) => {
  await ready.catch(() => undefined);
  scope.postMessage(handle(ev.data));
};
