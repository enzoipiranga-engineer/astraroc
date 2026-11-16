/**
 * Área financeira: lista de materiais (BOM) gerada do projeto, orçamento por subsistema,
 * receitas, custo por voo e fronteira de Pareto. Lógica pura, sem física.
 */
import type { Design, ElecComponent, PartKind } from "./types";

export type Currency = "BRL" | "USD" | "EUR";
export type Subsystem = "estrutura" | "propulsao" | "avionica" | "recuperacao" | "carga" | "lancador" | "outros";
export type PurchaseStatus = "planejado" | "pedido" | "recebido";

export const SUBSYSTEMS: { id: Subsystem; label: string }[] = [
  { id: "estrutura", label: "Estrutura" },
  { id: "propulsao", label: "Propulsão" },
  { id: "avionica", label: "Aviônica" },
  { id: "recuperacao", label: "Recuperação" },
  { id: "carga", label: "Carga útil" },
  { id: "lancador", label: "Lançador" },
  { id: "outros", label: "Outros" },
];

/** Dados de compra de um item (o que o usuário informa). */
export interface BomEntry {
  /** "part:<id>", "elec:<id>", "motor", "launcher:<n>" ou "extra:<n>". */
  key: string;
  /** Nome (itens extras). */
  name?: string;
  /** Preço unitário na moeda do item. Para peças sem valor aqui vale `Part.cost` (R$). */
  price?: number | null;
  currency?: Currency;
  qty?: number;
  supplier?: string;
  /** Frete, na moeda do item. */
  shipping?: number;
  /** Impostos e taxas de importação, % sobre preço + frete. */
  taxPct?: number;
  status?: PurchaseStatus;
  /** Gasto real, R$. */
  actual?: number | null;
  invoice?: string;
  critical?: boolean;
  /** Gasto a cada voo (motor, e-match). */
  consumable?: boolean;
  subsystem?: Subsystem;
}

export type RevenueKind = "patrocinio" | "parceria" | "edital" | "doacao";

export interface Revenue {
  id: string;
  source: string;
  kind: RevenueKind;
  amount: number;
  currency: Currency;
  status: "prometido" | "recebido";
  date?: string;
  notes?: string;
}

export interface Rates {
  /** R$ por 1 US$. */
  USD: number;
  /** R$ por 1 €. */
  EUR: number;
  /** Data da cotação (AAAA-MM-DD). */
  date: string;
  source: string;
}

export interface Finance {
  rates: Rates | null;
  /** Teto total, R$. */
  cap?: number | null;
  /** Teto por subsistema, R$. */
  caps: Partial<Record<Subsystem, number>>;
  entries: BomEntry[];
  revenues: Revenue[];
  /** Preço por kg de material, R$/kg (informado pelo usuário), para estimar peças sem preço. */
  materialPrices: Record<string, number>;
  /** Voos planejados com as peças reutilizáveis. */
  flightsPlanned: number;
}

export const emptyFinance = (): Finance => ({ rates: null, cap: null, caps: {}, entries: [], revenues: [], materialPrices: {}, flightsPlanned: 1 });

/** Lê o campo `finance` do projeto, completando o que faltar. */
export function financeOf(d: Design): Finance {
  const f = (d as Design & { finance?: Partial<Finance> }).finance;
  return { ...emptyFinance(), ...(f ?? {}) };
}

export interface BomLine {
  key: string;
  name: string;
  subsystem: Subsystem;
  qty: number;
  /** Preço unitário na moeda do item. */
  unitPrice: number | null;
  currency: Currency;
  shipping: number;
  taxPct: number;
  /** Total previsto em R$ (preço·qtd + frete)·(1 + imposto). */
  totalBrl: number;
  /** Preço estimado pela massa × R$/kg do material. */
  estimated: boolean;
  /** Sem preço. */
  missing: boolean;
  status: PurchaseStatus;
  actualBrl: number | null;
  supplier: string;
  invoice: string;
  critical: boolean;
  consumable: boolean;
}

/** Converte para R$. Sem cotação, só R$ é convertível. */
export function toBrl(v: number, c: Currency, rates: Rates | null): number {
  if (c === "BRL") return v;
  return rates ? v * rates[c] : Number.NaN;
}

const STRUCTURE = new Set<PartKind["type"]>(["noseCone", "bodyTube", "transition", "coupler", "bulkhead", "centeringRing", "finSet"]);

/** Subsistema de cada peça pelo papel no projeto. */
export function subsystemOfPart(d: Design, partId: string): Subsystem {
  const p = d.parts.find((x) => x.id === partId);
  if (!p) return "outros";
  if (p.kind.type === "parachute" || p.kind.type === "shockCord") return "recuperacao";
  if (d.payloads.some((x) => x.part === partId)) return "carga";
  if (d.electronics.components.some((c) => c.part === partId)) return "avionica";
  if (p.kind.type === "motorMount") return "propulsao";
  if (STRUCTURE.has(p.kind.type)) return "estrutura";
  return "outros";
}

/** Nome do motor do projeto. */
function motorName(d: Design): string {
  const m = d.motor;
  if (m.type === "eng") return `Motor ${m.text.split(/\r?\n/).find((l) => l.trim() && !l.trim().startsWith(";"))?.trim().split(/\s+/)[0] ?? ""}`.trim();
  if (m.type === "water") return "Garrafa e água (foguete de água)";
  return "";
}

/**
 * Gera a lista de materiais do projeto: peças, componentes eletrônicos, motor, lançadores
 * e itens extras. `masses` é a massa de cada peça (kg, na ordem de `design.parts`).
 */
export function buildBom(d: Design, lib: ElecComponent[], masses: number[] = []): BomLine[] {
  const f = financeOf(d);
  const entry = (key: string) => f.entries.find((e) => e.key === key) ?? { key };
  const lines: BomLine[] = [];
  const push = (key: string, name: string, sub: Subsystem, fallbackBrl: number | null, estimated: boolean, consumableDefault: boolean) => {
    const e = entry(key);
    const currency = e.currency ?? "BRL";
    const unit = e.price ?? (fallbackBrl !== null && currency === "BRL" ? fallbackBrl : null);
    const qty = e.qty ?? 1;
    const shipping = e.shipping ?? 0;
    const taxPct = e.taxPct ?? 0;
    const total = unit === null ? 0 : toBrl((unit * qty + shipping) * (1 + taxPct / 100), currency, f.rates);
    lines.push({
      key,
      name: e.name ?? name,
      subsystem: e.subsystem ?? sub,
      qty,
      unitPrice: unit,
      currency,
      shipping,
      taxPct,
      totalBrl: Number.isFinite(total) ? total : 0,
      estimated: e.price == null && estimated,
      missing: unit === null || !Number.isFinite(total),
      status: e.status ?? "planejado",
      actualBrl: e.actual ?? null,
      supplier: e.supplier ?? "",
      invoice: e.invoice ?? "",
      critical: e.critical ?? false,
      consumable: e.consumable ?? consumableDefault,
    });
  };
  d.parts.forEach((p, i) => {
    const perKg = f.materialPrices[p.material];
    const est = p.cost == null && perKg !== undefined && masses[i] !== undefined ? masses[i]! * perKg : null;
    push(`part:${p.id}`, p.name, subsystemOfPart(d, p.id), p.cost ?? est, p.cost == null && est !== null, false);
  });
  for (const c of d.electronics.components) {
    const l = lib.find((x) => x.id === c.lib);
    push(`elec:${c.id}`, `${l?.name ?? c.lib} (${c.id})`, l?.category === "pirotecnia" && l.ematch ? "recuperacao" : "avionica", null, false, Boolean(l?.ematch));
  }
  const mn = motorName(d);
  if (mn) push("motor", mn, "propulsao", null, false, d.motor.type === "eng");
  d.launcher.forEach((l, i) => push(`launcher:${i}`, `Lançador: ${"label" in l && typeof l.label === "string" ? l.label : l.type}`, "lancador", null, false, false));
  for (const e of f.entries.filter((x) => x.key.startsWith("extra:"))) push(e.key, e.name ?? "Item", e.subsystem ?? "outros", null, false, false);
  return lines;
}

export interface SubsystemBudget {
  id: Subsystem;
  label: string;
  planned: number;
  actual: number;
  cap: number | null;
}

export interface Budget {
  subsystems: SubsystemBudget[];
  planned: number;
  actual: number;
  cap: number | null;
  revenueReceived: number;
  revenuePromised: number;
  /** Receitas recebidas − gasto real. */
  balance: number;
  /** Todas as receitas − previsto. */
  projected: number;
  costPerFlight: number;
  missing: number;
  alerts: string[];
}

const brl = (v: number) => `R$ ${v.toFixed(2).replace(".", ",")}`;

/** Orçamento por subsistema, saldo, custo por voo e alertas. */
export function budget(d: Design, lines: BomLine[]): Budget {
  const f = financeOf(d);
  const alerts: string[] = [];
  const subsystems = SUBSYSTEMS.map((s) => {
    const ls = lines.filter((l) => l.subsystem === s.id);
    const planned = ls.reduce((a, l) => a + l.totalBrl, 0);
    const actual = ls.reduce((a, l) => a + (l.actualBrl ?? 0), 0);
    const cap = f.caps[s.id] ?? null;
    if (cap !== null && planned > cap) alerts.push(`${s.label}: previsto ${brl(planned)} passa do teto de ${brl(cap)}.`);
    if (cap !== null && actual > cap) alerts.push(`${s.label}: gasto real ${brl(actual)} já passou do teto de ${brl(cap)}.`);
    return { id: s.id, label: s.label, planned, actual, cap };
  }).filter((s) => s.planned > 0 || s.actual > 0 || s.cap !== null);
  const planned = lines.reduce((a, l) => a + l.totalBrl, 0);
  const actual = lines.reduce((a, l) => a + (l.actualBrl ?? 0), 0);
  const cap = f.cap ?? null;
  if (cap !== null && planned > cap) alerts.unshift(`Previsto ${brl(planned)} passa do teto total de ${brl(cap)} (${brl(planned - cap)} acima).`);
  if (cap !== null && actual > cap) alerts.unshift(`Gasto real ${brl(actual)} passou do teto total de ${brl(cap)}.`);
  for (const l of lines) {
    if (l.actualBrl !== null && l.totalBrl > 0 && l.actualBrl > 1.1 * l.totalBrl) alerts.push(`${l.name}: gasto real ${brl(l.actualBrl)} mais de 10 % acima do previsto (${brl(l.totalBrl)}).`);
    if (l.critical && l.status !== "recebido") alerts.push(`Item crítico ainda não recebido: ${l.name} (${l.status}).`);
    if (l.currency !== "BRL" && !f.rates) alerts.push(`${l.name} está em ${l.currency}: informe a cotação.`);
  }
  const rev = (st: Revenue["status"] | null) => f.revenues.filter((r) => st === null || r.status === st).reduce((a, r) => a + (toBrl(r.amount, r.currency, f.rates) || 0), 0);
  const consumables = lines.filter((l) => l.consumable).reduce((a, l) => a + l.totalBrl, 0);
  const reusable = planned - consumables;
  const missing = lines.filter((l) => l.missing).length;
  if (missing) alerts.push(`${missing} ${missing === 1 ? "item sem preço" : "itens sem preço"}: o total está incompleto.`);
  return {
    subsystems,
    planned,
    actual,
    cap,
    revenueReceived: rev("recebido"),
    revenuePromised: rev("prometido"),
    balance: rev("recebido") - actual,
    projected: rev(null) - planned,
    costPerFlight: consumables + reusable / Math.max(1, f.flightsPlanned),
    missing,
    alerts,
  };
}

/** Lê a resposta da API Frankfurter (taxas do BCE): `{ base: "USD", date, rates: { BRL, EUR } }`. */
export function ratesFromFrankfurter(json: unknown): Rates {
  const j = json as { base?: string; date?: string; rates?: Record<string, number> };
  const brl = j.rates?.BRL;
  const eur = j.rates?.EUR;
  if (j.base !== "USD" || typeof brl !== "number" || typeof eur !== "number" || !j.date) throw new Error("Resposta de câmbio em formato inesperado.");
  return { USD: brl, EUR: brl / eur, date: j.date, source: "BCE (via Frankfurter)" };
}

export interface ParetoPoint {
  cost: number;
  apogee: number;
  safety: number;
}

/**
 * Pontos não dominados: menor custo, maior apogeu e maior fator de segurança. Um ponto é
 * dominado se outro é pelo menos tão bom em tudo e melhor em algo.
 */
export function paretoFront<T extends ParetoPoint>(pts: T[]): T[] {
  return pts.filter((a) => !pts.some((b) => b !== a && b.cost <= a.cost && b.apogee >= a.apogee && b.safety >= a.safety && (b.cost < a.cost || b.apogee > a.apogee || b.safety > a.safety)));
}

/** Atualiza (ou cria) os dados de compra de um item. */
export function setEntry(d: Design, key: string, patch: Partial<BomEntry>): Design {
  const f = financeOf(d);
  const has = f.entries.some((e) => e.key === key);
  const entries = has ? f.entries.map((e) => (e.key === key ? { ...e, ...patch } : e)) : [...f.entries, { key, ...patch }];
  return { ...d, finance: { ...f, entries } } as Design;
}

/** Atualiza os dados financeiros. */
export function setFinance(d: Design, patch: Partial<Finance>): Design {
  return { ...d, finance: { ...financeOf(d), ...patch } } as Design;
}
