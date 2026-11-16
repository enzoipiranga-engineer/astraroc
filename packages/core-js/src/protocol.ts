/**
 * Protocolo de mensagens entre a thread da UI e o Web Worker do núcleo.
 * Cada pedido carrega um `id`; a resposta devolve o mesmo `id`.
 */
import type { Design, EngineInfo, Material, MissionOutput, MissionRequest, SimOutput, TemplateEntry, Validation } from "./types";

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
  | { id: number; kind: "engines" };

export type CoreResult =
  | { kind: "info"; info: CoreInfo }
  | { kind: "templates"; templates: TemplateEntry[] }
  | { kind: "materials"; materials: Material[] }
  | { kind: "validate"; validation: Validation }
  | { kind: "simulate"; output: SimOutput }
  | { kind: "mission"; mission: MissionOutput }
  | { kind: "engines"; engines: EngineInfo[] };

export type CoreResponse =
  | ({ id: number; ok: true } & CoreResult)
  | { id: number; ok: false; error: string };

/** Distribui `Omit` sobre a união (o `Omit` comum colapsa uniões). */
export type RequestBody = CoreRequest extends infer R ? (R extends { id: number } ? Omit<R, "id"> : never) : never;
