/** Estado do editor: histórico (desfazer/refazer), autosave e fábrica de peças. */
import type { Design, Joint, JointType, Part, PartType } from "@astra/core-js";

const KEY = "astra.design.v1";

export function loadSaved(): Design | null {
  try {
    const raw = localStorage.getItem(KEY);
    return raw ? (JSON.parse(raw) as Design) : null;
  } catch {
    return null;
  }
}

export function save(d: Design) {
  try {
    localStorage.setItem(KEY, JSON.stringify(d));
  } catch {
    /* armazenamento indisponível: segue sem autosave */
  }
}

export interface History {
  past: Design[];
  present: Design;
  future: Design[];
}

export const historyOf = (d: Design): History => ({ past: [], present: d, future: [] });

export function commit(h: History, next: Design): History {
  return { past: [...h.past.slice(-99), h.present], present: next, future: [] };
}

export function undo(h: History): History {
  const prev = h.past.at(-1);
  return prev ? { past: h.past.slice(0, -1), present: prev, future: [h.present, ...h.future] } : h;
}

export function redo(h: History): History {
  const next = h.future[0];
  return next ? { past: [...h.past, h.present], present: next, future: h.future.slice(1) } : h;
}

export function uniqueId(base: string, taken: string[]): string {
  if (!taken.includes(base)) return base;
  let i = 2;
  while (taken.includes(`${base}-${i}`)) i++;
  return `${base}-${i}`;
}

/** Diâmetro do primeiro tubo do projeto (para dimensionar peças novas). */
function bodyDiameter(d: Design): number {
  for (const p of d.parts) if (p.kind.type === "bodyTube") return p.kind.outerDiameter;
  return 0.056;
}

/** Fim da casca externa (para pôr peças externas novas no fim). */
function aftEnd(d: Design): number {
  let x = 0;
  for (const p of d.parts) {
    const k = p.kind;
    if (k.type === "noseCone") x = Math.max(x, p.position + k.length);
    if (k.type === "bodyTube" || k.type === "transition") x = Math.max(x, p.position + k.length);
  }
  return x;
}

export function newPart(d: Design, type: PartType): Part {
  const D = bodyDiameter(d);
  const inner = D - 0.003;
  const x = aftEnd(d);
  const ids = d.parts.map((p) => p.id);
  const mk = (id: string, name: string, material: string, position: number, kind: Part["kind"]): Part => ({
    id: uniqueId(id, ids),
    name,
    material,
    position,
    massOverride: null,
    cost: null,
    kind,
  });
  switch (type) {
    case "noseCone":
      return mk("coifa", "Coifa", "petg", 0, { type, shape: "tangentOgive", length: 4 * D, diameter: D, wall: 0.0015, shoulderLength: D });
    case "bodyTube":
      return mk("tubo", "Tubo", "papel-fenolico", x, { type, length: 0.3, outerDiameter: D, wall: 0.0015, ventHoles: 0 });
    case "transition":
      return mk("transicao", "Transição", "petg", x, { type, length: D, dFore: D, dAft: 0.7 * D, wall: 0.0015 });
    case "coupler":
      return mk("luva", "Luva", "papel-fenolico", Math.max(0, x - 0.075), { type, length: 0.15, outerDiameter: inner, wall: 0.0015 });
    case "motorMount":
      return mk("tubo-motor", "Tubo do motor", "papel-fenolico", Math.max(0, x - 0.2), { type, length: 0.2, outerDiameter: 0.031, wall: 0.001 });
    case "bulkhead":
      return mk("anteparo", "Anteparo", "compensado-betula", Math.max(0, x / 2), { type, diameter: inner, thickness: 0.006 });
    case "centeringRing":
      return mk("anel", "Anel de centragem", "compensado-betula", Math.max(0, x - 0.1), { type, outerDiameter: inner, innerDiameter: 0.031, thickness: 0.003 });
    case "finSet":
      return mk("aletas", "Aletas", "g10-fr4", Math.max(0, x - 0.1), { type, count: 3, rootChord: 0.1, tipChord: 0.045, sweep: 0.06, span: 1.2 * D, thickness: 0.002, profile: "rounded" });
    case "parachute":
      return mk("paraquedas", "Paraquedas", "ripstop-1.1", Math.max(0, x / 3), { type, diameter: 0.6, cd: 0.8, packedLength: 0.08, packedDiameter: 0.8 * inner, deploy: { type: "apogee", delay: 1 }, separationJoint: null, ejectionCharge: 0.0006 });
    case "shockCord":
      return mk("cordao", "Cordão de choque", "kevlar-tecido", Math.max(0, x / 3), { type, length: 2, linearDensity: 0.012, strength: 1500 });
    case "mass":
      return mk("componente", "Componente", "g10-fr4", Math.max(0, x / 2), { type, mass: 0.03, length: 0.05, diameter: 0.6 * inner, needsStaticPort: false });
  }
}

export function newJoint(d: Design, a: string, b: string, type: JointType): Joint {
  const id = uniqueId(`j-${a}-${b}`, d.joints.map((j) => j.id));
  const kind: Joint["kind"] =
    type === "adhesive"
      ? { type, adhesive: "epoxi-estrutural", area: null, fillet: null, throughWall: false }
      : type === "screws"
        ? { type, count: 4, diameter: 0.003, material: "aco-parafuso-8.8" }
        : type === "shearPins"
          ? { type, count: 2, diameter: 0.0016, material: "nylon-66" }
          : type === "frictionFit"
            ? { type, holdingForce: 20 }
            : type === "latch"
              ? { type, holdingForce: 50 }
              : type === "baySeat"
                ? { type, bayJoint: d.joints.find((j) => j.kind.type === "shearPins")?.id ?? "" }
                : { type, strength: 1000 };
  return { id, a, b, kind };
}
