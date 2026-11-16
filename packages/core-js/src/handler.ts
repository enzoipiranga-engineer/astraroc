/**
 * Atende um pedido chamando o núcleo WASM já inicializado. Separado do worker para
 * poder ser testado em Node.
 */
import {
  checkPlugin,
  coreVersion,
  launcherPlugins,
  designTemplates,
  electronicsLibrary,
  exampleFirmware,
  engineLibrary,
  simulateMission,
  materialsJson,
  selfCheck,
  simulateDesign,
  validateDesign,
} from "../wasm/astra_wasm.js";
import type { CoreRequest, CoreResponse, ExampleFirmware } from "./protocol";
import { fly } from "./avr/fly";
import type { Design, ElecComponent, EngineInfo, Material, MissionOutput, PluginCheck, PluginSpec, SimOutput, TemplateEntry, Validation } from "./types";

export function handle(req: CoreRequest): CoreResponse {
  try {
    switch (req.kind) {
      case "info":
        return { id: req.id, ok: true, kind: "info", info: { version: coreVersion(), selfCheckError: selfCheck() } };
      case "templates":
        return { id: req.id, ok: true, kind: "templates", templates: JSON.parse(designTemplates()) as TemplateEntry[] };
      case "materials":
        return {
          id: req.id,
          ok: true,
          kind: "materials",
          materials: (JSON.parse(materialsJson()) as { materials: Material[] }).materials,
        };
      case "validate":
        return {
          id: req.id,
          ok: true,
          kind: "validate",
          validation: JSON.parse(validateDesign(JSON.stringify(req.design), req.withFlight)) as Validation,
        };
      case "simulate":
        return {
          id: req.id,
          ok: true,
          kind: "simulate",
          output: JSON.parse(simulateDesign(JSON.stringify(req.design))) as SimOutput,
        };
      case "mission":
        return { id: req.id, ok: true, kind: "mission", mission: JSON.parse(simulateMission(JSON.stringify(req.request))) as MissionOutput };
      case "plugins":
        return { id: req.id, ok: true, kind: "plugins", plugins: JSON.parse(launcherPlugins()) as PluginSpec[] };
      case "checkPlugin":
        return { id: req.id, ok: true, kind: "checkPlugin", check: JSON.parse(checkPlugin(JSON.stringify(req.spec))) as PluginCheck };
      case "electronics":
        return {
          id: req.id,
          ok: true,
          kind: "electronics",
          library: JSON.parse(electronicsLibrary()) as ElecComponent[],
          firmware: JSON.parse(exampleFirmware()) as ExampleFirmware,
        };
      case "fly": {
        const validation = JSON.parse(validateDesign(JSON.stringify(req.design), false)) as Validation;
        const library = JSON.parse(electronicsLibrary()) as ElecComponent[];
        const sim = (d: Design) => JSON.parse(simulateDesign(JSON.stringify(d))) as SimOutput;
        return { id: req.id, ok: true, kind: "fly", result: fly(req.design, validation, library, sim, req.seed === undefined ? {} : { seed: req.seed }) };
      }
      case "engines":
        return { id: req.id, ok: true, kind: "engines", engines: JSON.parse(engineLibrary()) as EngineInfo[] };
    }
  } catch (e) {
    return { id: req.id, ok: false, error: e instanceof Error ? e.message : String(e) };
  }
}
