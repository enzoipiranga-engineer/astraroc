/**
 * Protocolo de mensagens entre a thread da UI e o Web Worker do núcleo.
 * Cada pedido carrega um `id`; a resposta devolve o mesmo `id`.
 */
import type { Design, EngineInfo, Material, MissionOutput, MissionRequest, PluginCheck, PluginSpec, SimOutput, TemplateEntry, Validation, ElecComponent } from "./types";
import type { FlyResult } from "./avr/fly";

export interface CoreInfo {
  /** Versão do núcleo Rust (semver). */
  version: string;
  /** Erro (m) do autoteste numérico de ida e volta; esperado < 1e-8. */
  selfCheckError: number;
}

export type CoreRequest =
  | { id: number; kind: "info" }
  | { id: number; kind: "templates" }
  | { id: number; kind: "materials" }
  | { id: number; kind: "validate"; design: Design; withFlight: boolean }
  | { id: number; kind: "simulate"; design: Design }
  | { id: number; kind: "mission"; request: MissionRequest }
  | { id: number; kind: "engines" }
  | { id: number; kind: "plugins" }
  | { id: number; kind: "checkPlugin"; spec: PluginSpec }
  | { id: number; kind: "electronics" }
  | { id: number; kind: "fly"; design: Design; seed?: number };

export type CoreResult =
  | { kind: "info"; info: CoreInfo }
  | { kind: "templates"; templates: TemplateEntry[] }
  | { kind: "materials"; materials: Material[] }
  | { kind: "validate"; validation: Validation }
  | { kind: "simulate"; output: SimOutput }
  | { kind: "mission"; mission: MissionOutput }
  | { kind: "engines"; engines: EngineInfo[] }
  | { kind: "plugins"; plugins: PluginSpec[] }
  | { kind: "checkPlugin"; check: PluginCheck }
  | { kind: "electronics"; library: ElecComponent[]; firmware: ExampleFirmware }
  | { kind: "fly"; result: FlyResult };

/** Firmware de exemplo que acompanha o ASTRA. */
export interface ExampleFirmware {
  name: string;
  hex: string;
  source: string;
}

export type CoreResponse =
  | ({ id: number; ok: true } & CoreResult)
  | { id: number; ok: false; error: string };

/** Distribui `Omit` sobre a união (o `Omit` comum colapsa uniões). */
export type RequestBody = CoreRequest extends infer R ? (R extends { id: number } ? Omit<R, "id"> : never) : never;
