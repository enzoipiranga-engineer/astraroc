import type { CoreInfo, CoreRequest, CoreResponse } from "./protocol";

/** O mínimo de `Worker` que o cliente usa (permite testar sem navegador). */
export interface WorkerLike {
  postMessage(msg: CoreRequest): void;
  onmessage: ((ev: { data: CoreResponse }) => void) | null;
  onerror: ((ev: { message?: string }) => void) | null;
}

type Pending = { resolve: (r: CoreResponse) => void; reject: (e: Error) => void };

/** Cliente assíncrono do núcleo: transforma mensagens do worker em Promises. */
export class CoreClient {
  private nextId = 1;
  private readonly pending = new Map<number, Pending>();

  private readonly worker: WorkerLike;

  constructor(worker: WorkerLike | Worker) {
    // `Worker` entrega `MessageEvent`; só lemos `data`, cujo formato o protocolo garante.
    this.worker = worker as WorkerLike;
    this.worker.onmessage = (ev) => {
      const p = this.pending.get(ev.data.id);
      if (!p) return;
      this.pending.delete(ev.data.id);
      p.resolve(ev.data);
    };
    this.worker.onerror = (ev) => {
      const err = new Error(ev.message ?? "Falha no worker do núcleo");
      for (const p of this.pending.values()) p.reject(err);
      this.pending.clear();
    };
  }

  private call(kind: CoreRequest["kind"]): Promise<CoreResponse> {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.worker.postMessage({ id, kind });
    });
  }

  /** Versão e autoteste do núcleo carregado. */
  async info(): Promise<CoreInfo> {
    const res = await this.call("info");
    if (!res.ok) throw new Error(res.error);
    return res.info;
  }
}
