import type { CoreInfo, CoreRequest, CoreResponse, CoreResult, RequestBody } from "./protocol";
import type { Design, EngineInfo, Material, MissionOutput, MissionRequest, PluginCheck, PluginSpec, SimOutput, TemplateEntry, Validation } from "./types";

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

  private async call<K extends CoreResult["kind"]>(body: RequestBody & { kind: K }): Promise<Extract<CoreResult, { kind: K }>> {
    const id = this.nextId++;
    const res = await new Promise<CoreResponse>((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.worker.postMessage({ ...body, id } as CoreRequest);
    });
    if (!res.ok) throw new Error(res.error);
    return res as unknown as Extract<CoreResult, { kind: K }>;
  }

  /** Versão e autoteste do núcleo carregado. */
  async info(): Promise<CoreInfo> {
    return (await this.call({ kind: "info" })).info;
  }

  /** Projetos de exemplo. */
  async templates(): Promise<TemplateEntry[]> {
    return (await this.call({ kind: "templates" })).templates;
  }

  /** Banco de materiais embutido. */
  async materials(): Promise<Material[]> {
    return (await this.call({ kind: "materials" })).materials;
  }

  /** Verifica o projeto; com `withFlight`, inclui as cargas do voo sem falhas. */
  async validate(design: Design, withFlight: boolean): Promise<Validation> {
    return (await this.call({ kind: "validate", design, withFlight })).validation;
  }

  /** Simula uma missão orbital. */
  async mission(request: MissionRequest): Promise<MissionOutput> {
    return (await this.call({ kind: "mission", request })).mission;
  }

  /** Plugins de lançador que acompanham o ASTRA. */
  async plugins(): Promise<PluginSpec[]> {
    return (await this.call({ kind: "plugins" })).plugins;
  }

  /** Verificação automática de um plugin. */
  async checkPlugin(spec: PluginSpec): Promise<PluginCheck> {
    return (await this.call({ kind: "checkPlugin", spec })).check;
  }

  /** Biblioteca de motores líquidos. */
  async engines(): Promise<EngineInfo[]> {
    return (await this.call({ kind: "engines" })).engines;
  }

  /** Simula o voo com falhas físicas. */
  async simulate(design: Design): Promise<SimOutput> {
    return (await this.call({ kind: "simulate", design })).output;
  }
}
