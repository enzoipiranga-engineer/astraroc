/**
 * Campos editáveis de cada tipo de peça e junta, com a unidade mostrada ao usuário.
 * O projeto guarda tudo em SI; aqui fica só a conversão para exibição.
 */
import type { JointType, PartType } from "@astra/core-js";

export type Unit = "mm" | "m" | "g" | "kg" | "N" | "count" | "ratio" | "bar" | "L" | "s" | "deg" | "kgm" | "brl";

const factor: Record<Unit, number> = {
  mm: 1000,
  m: 1,
  g: 1000,
  kg: 1,
  N: 1,
  count: 1,
  ratio: 1,
  bar: 1e-5,
  L: 1000,
  s: 1,
  deg: 1,
  kgm: 1,
  brl: 1,
};

export const unitLabel: Record<Unit, string> = {
  mm: "mm",
  m: "m",
  g: "g",
  kg: "kg",
  N: "N",
  count: "",
  ratio: "",
  bar: "bar",
  L: "L",
  s: "s",
  deg: "°",
  kgm: "kg/m",
  brl: "R$",
};

export const toDisplay = (si: number, u: Unit) => si * factor[u];
export const fromDisplay = (v: number, u: Unit) => v / factor[u];

export interface FieldSpec {
  key: string;
  label: string;
  unit: Unit;
  help: string;
}

const f = (key: string, label: string, unit: Unit, help: string): FieldSpec => ({ key, label, unit, help });

export const partFields: Record<PartType, FieldSpec[]> = {
  noseCone: [
    f("length", "Comprimento", "mm", "Comprimento exposto da coifa, sem o ombro."),
    f("diameter", "Diâmetro da base", "mm", "Deve ser igual ao diâmetro externo do tubo."),
    f("wall", "Parede", "mm", "Espessura da casca."),
    f("shoulderLength", "Ombro", "mm", "Parte que entra no tubo. Ombro curto deixa a coifa folgada."),
  ],
  bodyTube: [
    f("length", "Comprimento", "mm", ""),
    f("outerDiameter", "Diâmetro externo", "mm", ""),
    f("wall", "Parede", "mm", "Espessura da parede; entra na massa e na flambagem."),
    f("ventHoles", "Furos de respiro", "count", "Furos de pressão estática / alívio. Sem eles a baia fica fechada."),
  ],
  transition: [
    f("length", "Comprimento", "mm", ""),
    f("dFore", "Diâmetro dianteiro", "mm", ""),
    f("dAft", "Diâmetro traseiro", "mm", "Menor que o dianteiro: boattail."),
    f("wall", "Parede", "mm", ""),
  ],
  coupler: [
    f("length", "Comprimento", "mm", "Luva que une dois tubos por dentro."),
    f("outerDiameter", "Diâmetro externo", "mm", "Igual ao diâmetro interno dos tubos."),
    f("wall", "Parede", "mm", ""),
  ],
  motorMount: [
    f("length", "Comprimento", "mm", ""),
    f("outerDiameter", "Diâmetro externo", "mm", ""),
    f("wall", "Parede", "mm", ""),
  ],
  bulkhead: [
    f("diameter", "Diâmetro", "mm", "Igual ao diâmetro interno do tubo ou da luva."),
    f("thickness", "Espessura", "mm", ""),
  ],
  centeringRing: [
    f("outerDiameter", "Diâmetro externo", "mm", "Igual ao interno do tubo."),
    f("innerDiameter", "Diâmetro interno", "mm", "Igual ao externo do tubo do motor."),
    f("thickness", "Espessura", "mm", ""),
  ],
  finSet: [
    f("count", "Número de aletas", "count", ""),
    f("rootChord", "Corda na raiz", "mm", ""),
    f("tipChord", "Corda na ponta", "mm", ""),
    f("sweep", "Recuo do bordo de ataque", "mm", "Quanto a ponta começa atrás da raiz."),
    f("span", "Envergadura", "mm", "Da raiz à ponta."),
    f("thickness", "Espessura", "mm", "Aleta fina demais entra em flutter."),
  ],
  parachute: [
    f("diameter", "Diâmetro do velame", "mm", ""),
    f("cd", "Cd do velame", "ratio", "Cerca de 0,75 a 0,8 para velame circular plano."),
    f("packedLength", "Comprimento dobrado", "mm", ""),
    f("packedDiameter", "Diâmetro dobrado", "mm", "Deve caber no tubo."),
    f("ejectionCharge", "Carga de ejeção", "g", "Pólvora negra. A força de ejeção precisa vencer a junta de separação."),
  ],
  shockCord: [
    f("length", "Comprimento", "m", ""),
    f("linearDensity", "Massa por metro", "kgm", ""),
    f("strength", "Carga de ruptura", "N", "Precisa aguentar a abertura do paraquedas."),
  ],
  mass: [
    f("mass", "Massa", "g", ""),
    f("length", "Comprimento", "mm", ""),
    f("diameter", "Diâmetro / largura", "mm", "Espaço ocupado; precisa caber no tubo."),
  ],
};

export const partTypeLabel: Record<PartType, string> = {
  noseCone: "Coifa",
  bodyTube: "Tubo",
  transition: "Transição",
  coupler: "Luva",
  motorMount: "Tubo do motor",
  bulkhead: "Anteparo",
  centeringRing: "Anel de centragem",
  finSet: "Aletas",
  parachute: "Paraquedas",
  shockCord: "Cordão de choque",
  mass: "Componente / massa",
};

export const jointFields: Record<JointType, FieldSpec[]> = {
  adhesive: [
    f("area", "Área colada (opcional)", "mm", "Em mm². Vazio: calculada pelo contato entre as peças."),
    f("fillet", "Raio do filete (aletas)", "mm", "Filete de cola na raiz da aleta."),
  ],
  screws: [f("count", "Quantidade", "count", ""), f("diameter", "Diâmetro", "mm", "")],
  shearPins: [
    f("count", "Quantidade", "count", ""),
    f("diameter", "Diâmetro efetivo", "mm", "Raiz da rosca: cerca de 1,6 mm para pino 2-56."),
  ],
  frictionFit: [f("holdingForce", "Força para soltar", "N", "Meça com um dinamômetro.")],
  latch: [f("holdingForce", "Força nominal", "N", "")],
  cord: [f("strength", "Carga de ruptura", "N", "")],
};

export const jointTypeLabel: Record<JointType, string> = {
  adhesive: "Colagem",
  screws: "Parafusos",
  shearPins: "Pinos de cisalhamento",
  frictionFit: "Encaixe por atrito",
  latch: "Trava / abraçadeira",
  cord: "Cordão",
};

export const fmt = (v: number, digits = 1) =>
  Number.isFinite(v) ? v.toLocaleString("pt-BR", { maximumFractionDigits: digits, minimumFractionDigits: 0 }) : "—";
