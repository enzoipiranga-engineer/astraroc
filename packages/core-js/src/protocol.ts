/**
 * Protocolo de mensagens entre a thread da UI e o Web Worker do núcleo.
 * Cada pedido carrega um `id`; a resposta devolve o mesmo `id`.
 */
export interface CoreInfo {
  /** Versão do núcleo Rust (semver). */
  version: string;
  /** Erro (m) do autoteste numérico de ida e volta; esperado < 1e-8. */
  selfCheckError: number;
}

export type CoreRequest = { id: number; kind: "info" };

export type CoreResponse =
  | { id: number; ok: true; kind: "info"; info: CoreInfo }
  | { id: number; ok: false; error: string };
