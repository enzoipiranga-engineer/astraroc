import { useMemo, useState } from "react";
import {
  SUBSYSTEMS,
  budget,
  buildBom,
  financeOf,
  paretoFront,
  ratesFromFrankfurter,
  setEntry,
  setFinance,
  type BomLine,
  type CoreClient,
  type Currency,
  type Design,
  type ElecComponent,
  type Material,
  type PurchaseStatus,
  type Revenue,
  type RevenueKind,
  type Subsystem,
  type Validation,
} from "@astra/core-js";
import { download, fileStem } from "../export";
import { csvBr, xlsx, type Cell } from "../xlsx";
import { locale, translateDocument } from "../i18n";

interface Props {
  design: Design;
  validation: Validation | null;
  library: ElecComponent[];
  materials: Material[];
  core: CoreClient;
  onChange: (d: Design) => void;
}

const money = (v: number | null | undefined) => (v === null || v === undefined || !Number.isFinite(v) ? "—" : `R$ ${v.toLocaleString(locale(), { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`);
const numIn = (v: string) => (v.trim() === "" ? null : Number(v.replace(",", ".")));
const KINDS: { id: RevenueKind; label: string }[] = [
  { id: "patrocinio", label: "Patrocínio" },
  { id: "parceria", label: "Parceria" },
  { id: "edital", label: "Edital" },
  { id: "doacao", label: "Doação" },
];
const STATUS: { id: PurchaseStatus; label: string }[] = [
  { id: "planejado", label: "planejado" },
  { id: "pedido", label: "pedido" },
  { id: "recebido", label: "recebido" },
];

/** Linhas da planilha da lista de materiais. */
function bomRows(lines: BomLine[]): Cell[][] {
  return [
    ["Item", "Subsistema", "Qtd", "Preço unit.", "Moeda", "Frete", "Impostos %", "Total previsto (R$)", "Gasto real (R$)", "Status", "Fornecedor", "Nota fiscal", "Crítico", "Consumível", "Observação"],
    ...lines.map((l) => [l.name, SUBSYSTEMS.find((s) => s.id === l.subsystem)?.label ?? l.subsystem, l.qty, l.unitPrice, l.currency, l.shipping, l.taxPct, l.totalBrl, l.actualBrl, l.status, l.supplier, l.invoice, l.critical ? "sim" : "", l.consumable ? "sim" : "", l.missing ? "sem preço" : l.estimated ? "estimado pelo material" : ""]),
  ];
}

export function FinancePanel({ design, validation, library, materials, core, onChange }: Props) {
  const f = financeOf(design);
  const masses = validation?.summary.partMasses ?? [];
  const lines = useMemo(() => buildBom(design, library, masses), [design, library, masses]);
  const b = useMemo(() => budget(design, lines), [design, lines]);
  const [fxErr, setFxErr] = useState("");
  const [open, setOpen] = useState(false);
  const set = (key: string, patch: Parameters<typeof setEntry>[2]) => {
    let d = setEntry(design, key, patch);
    // O preço em R$ de uma peça também vai para Part.cost (custo do resumo do projeto).
    if (key.startsWith("part:") && "price" in patch) {
      const cur = patch.currency ?? financeOf(d).entries.find((e) => e.key === key)?.currency ?? "BRL";
      if (cur === "BRL") d = { ...d, parts: d.parts.map((p) => (`part:${p.id}` === key ? { ...p, cost: patch.price ?? null } : p)) };
    }
    onChange(d);
  };
  const setF = (patch: Parameters<typeof setFinance>[1]) => onChange(setFinance(design, patch));
  const fetchRates = async () => {
    setFxErr("");
    for (const url of ["https://api.frankfurter.dev/v1/latest?base=USD&symbols=BRL,EUR", "https://api.frankfurter.app/latest?from=USD&to=BRL,EUR"]) {
      try {
        const r = await fetch(url);
        if (!r.ok) continue;
        setF({ rates: ratesFromFrankfurter(await r.json()) });
        return;
      } catch {
        /* tenta o próximo endereço */
      }
    }
    setFxErr("Não consegui buscar a cotação agora; informe os valores à mão.");
  };
  const usedMaterials = [...new Set(design.parts.map((p) => p.material))];
  const setRevenue = (id: string, patch: Partial<Revenue>) => setF({ revenues: f.revenues.map((r) => (r.id === id ? { ...r, ...patch } : r)) });
  const name = fileStem(design.name);
  const budgetRows: Cell[][] = [["Subsistema", "Previsto (R$)", "Gasto real (R$)", "Teto (R$)"], ...b.subsystems.map((s) => [s.label, s.planned, s.actual, s.cap]), ["Total", b.planned, b.actual, b.cap]];
  const revenueRows: Cell[][] = [["Fonte", "Tipo", "Valor", "Moeda", "Situação", "Data", "Observação"], ...f.revenues.map((r) => [r.source, KINDS.find((k) => k.id === r.kind)?.label ?? r.kind, r.amount, r.currency, r.status, r.date ?? "", r.notes ?? ""])];

  return (
    <section className="panel finance">
      <h2>
        Finanças <small>lista de materiais, orçamento e receitas</small>
      </h2>
      <div className="cards">
        <div className="card"><b>{money(b.planned)}</b><span>previsto{b.missing ? ` (${b.missing} sem preço)` : ""}</span></div>
        <div className="card"><b>{money(b.actual)}</b><span>gasto real</span></div>
        <div className="card"><b>{money(b.revenueReceived)}</b><span>receitas recebidas (+{money(b.revenuePromised)} prometidas)</span></div>
        <div className={`card ${b.balance < 0 ? "bad" : ""}`}><b>{money(b.balance)}</b><span>saldo (recebido − gasto)</span></div>
        <div className={`card ${b.projected < 0 ? "bad" : ""}`}><b>{money(b.projected)}</b><span>projetado (receitas − previsto)</span></div>
        <div className="card"><b>{money(b.costPerFlight)}</b><span>custo por voo ({f.flightsPlanned} {f.flightsPlanned === 1 ? "voo" : "voos"})</span></div>
      </div>
      {b.alerts.map((a) => (
        <p key={a} className="pcheck bad">{a}</p>
      ))}
      <div className="fin-controls">
        <label className="field">
          <span>Teto total (R$)</span>
          <input inputMode="decimal" defaultValue={f.cap ?? ""} key={`cap-${f.cap}`} onBlur={(e) => setF({ cap: numIn(e.target.value) })} placeholder="sem teto" />
        </label>
        <label className="field">
          <span>Voos planejados</span>
          <input type="number" min={1} value={f.flightsPlanned} onChange={(e) => setF({ flightsPlanned: Math.max(1, Number(e.target.value) || 1) })} />
        </label>
        <label className="field">
          <span>R$ por US$</span>
          <input inputMode="decimal" defaultValue={Number.isFinite(f.rates?.USD) ? f.rates!.USD : ""} key={`usd-${f.rates?.USD}`} onBlur={(e) => { const v = numIn(e.target.value); if (v) setF({ rates: { USD: v, EUR: f.rates?.EUR ?? Number.NaN, date: new Date().toISOString().slice(0, 10), source: "manual" } }); }} />
        </label>
        <label className="field">
          <span>R$ por €</span>
          <input inputMode="decimal" defaultValue={Number.isFinite(f.rates?.EUR) ? f.rates!.EUR : ""} key={`eur-${f.rates?.EUR}`} onBlur={(e) => { const v = numIn(e.target.value); if (v) setF({ rates: { USD: f.rates?.USD ?? Number.NaN, EUR: v, date: new Date().toISOString().slice(0, 10), source: "manual" } }); }} />
        </label>
        <button onClick={() => void fetchRates()}>Atualizar cotação (BCE)</button>
        {f.rates && <span className="hint">Cotação de {f.rates.date} ({f.rates.source})</span>}
        {fxErr && <span className="pcheck bad">{fxErr}</span>}
      </div>

      <h3>Orçamento por subsistema</h3>
      <table className="fin-budget">
        <thead>
          <tr><th>Subsistema</th><th>Previsto</th><th>Gasto</th><th>Teto</th><th /></tr>
        </thead>
        <tbody>
          {SUBSYSTEMS.map((s) => {
            const row = b.subsystems.find((x) => x.id === s.id);
            const cap = f.caps[s.id];
            const pct = cap ? Math.min(100, (100 * (row?.planned ?? 0)) / cap) : 0;
            return (
              <tr key={s.id}>
                <td>{s.label}</td>
                <td>{money(row?.planned ?? 0)}</td>
                <td>{money(row?.actual ?? 0)}</td>
                <td><input className="tiny" inputMode="decimal" defaultValue={cap ?? ""} key={`${s.id}-${cap}`} placeholder="—" onBlur={(e) => { const v = numIn(e.target.value); const caps = { ...f.caps }; if (v === null) delete caps[s.id]; else caps[s.id] = v; setF({ caps }); }} /></td>
                <td className="bar-cell">{cap ? <div className={`bar ${(row?.planned ?? 0) > cap ? "over" : ""}`} style={{ width: `${pct}%` }} /> : null}</td>
              </tr>
            );
          })}
        </tbody>
      </table>

      <details open={open} onToggle={(e) => setOpen((e.target as HTMLDetailsElement).open)}>
        <summary>
          <h3 className="inline">Lista de materiais ({lines.length} itens)</h3>
        </summary>
        <div className="bom-wrap">
          <table className="bom">
            <thead>
              <tr><th>Item</th><th>Subsistema</th><th>Qtd</th><th>Preço</th><th>Moeda</th><th>Frete</th><th>Imp. %</th><th>Total</th><th>Status</th><th>Gasto real</th><th>Fornecedor</th><th>NF</th><th title="Item crítico">!</th><th title="Consumível (gasto a cada voo)">♻</th></tr>
            </thead>
            <tbody>
              {lines.map((l) => (
                <tr key={l.key} className={l.missing ? "missing" : ""}>
                  <td>{l.key.startsWith("extra:") ? <input defaultValue={l.name} onBlur={(e) => set(l.key, { name: e.target.value })} /> : l.name}</td>
                  <td>
                    <select value={l.subsystem} onChange={(e) => set(l.key, { subsystem: e.target.value as Subsystem })}>
                      {SUBSYSTEMS.map((s) => <option key={s.id} value={s.id}>{s.label}</option>)}
                    </select>
                  </td>
                  <td><input className="tiny" type="number" min={0} value={l.qty} onChange={(e) => set(l.key, { qty: Number(e.target.value) || 0 })} /></td>
                  <td><input className="tiny" inputMode="decimal" defaultValue={l.estimated ? "" : (l.unitPrice ?? "")} key={`${l.key}-${l.unitPrice}-${l.estimated}`} placeholder={l.estimated && l.unitPrice !== null ? `≈ ${l.unitPrice.toFixed(2)}` : "—"} onBlur={(e) => { const v = numIn(e.target.value); if (v !== (l.estimated ? null : l.unitPrice)) set(l.key, { price: v }); }} /></td>
                  <td>
                    <select value={l.currency} onChange={(e) => set(l.key, { currency: e.target.value as Currency })}>
                      <option>BRL</option><option>USD</option><option>EUR</option>
                    </select>
                  </td>
                  <td><input className="tiny" inputMode="decimal" defaultValue={l.shipping || ""} key={`${l.key}-s-${l.shipping}`} onBlur={(e) => set(l.key, { shipping: numIn(e.target.value) ?? 0 })} /></td>
                  <td><input className="tiny" inputMode="decimal" defaultValue={l.taxPct || ""} key={`${l.key}-t-${l.taxPct}`} onBlur={(e) => set(l.key, { taxPct: numIn(e.target.value) ?? 0 })} /></td>
                  <td className="num">{l.missing ? "—" : money(l.totalBrl)}{l.estimated && <small> est.</small>}</td>
                  <td>
                    <select value={l.status} onChange={(e) => set(l.key, { status: e.target.value as PurchaseStatus })}>
                      {STATUS.map((s) => <option key={s.id} value={s.id}>{s.label}</option>)}
                    </select>
                  </td>
                  <td><input className="tiny" inputMode="decimal" defaultValue={l.actualBrl ?? ""} key={`${l.key}-a-${l.actualBrl}`} onBlur={(e) => set(l.key, { actual: numIn(e.target.value) })} /></td>
                  <td><input defaultValue={l.supplier} onBlur={(e) => set(l.key, { supplier: e.target.value })} /></td>
                  <td><input className="tiny" defaultValue={l.invoice} onBlur={(e) => set(l.key, { invoice: e.target.value })} /></td>
                  <td><input type="checkbox" checked={l.critical} onChange={(e) => set(l.key, { critical: e.target.checked })} /></td>
                  <td><input type="checkbox" checked={l.consumable} onChange={(e) => set(l.key, { consumable: e.target.checked })} /></td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <button onClick={() => { let n = 1; while (f.entries.some((e) => e.key === `extra:${n}`)) n++; set(`extra:${n}`, { name: "Novo item", subsystem: "outros" }); }}>Acrescentar item</button>
        <h3>Preço dos materiais (R$/kg)</h3>
        <p className="hint">Peças sem preço são estimadas pela massa × preço do material. Trocar o material então muda custo, massa e apogeu juntos.</p>
        <div className="row mats">
          {usedMaterials.map((m) => (
            <label key={m} className="field">
              <span>{materials.find((x) => x.id === m)?.name ?? m}</span>
              <input inputMode="decimal" defaultValue={f.materialPrices[m] ?? ""} key={`${m}-${f.materialPrices[m]}`} placeholder="R$/kg" onBlur={(e) => { const v = numIn(e.target.value); const mp = { ...f.materialPrices }; if (v === null) delete mp[m]; else mp[m] = v; setF({ materialPrices: mp }); }} />
            </label>
          ))}
        </div>
      </details>

      <h3>Receitas</h3>
      <table className="fin-rev">
        <tbody>
          {f.revenues.map((r) => (
            <tr key={r.id}>
              <td><input defaultValue={r.source} onBlur={(e) => setRevenue(r.id, { source: e.target.value })} /></td>
              <td><select value={r.kind} onChange={(e) => setRevenue(r.id, { kind: e.target.value as RevenueKind })}>{KINDS.map((k) => <option key={k.id} value={k.id}>{k.label}</option>)}</select></td>
              <td><input className="tiny" inputMode="decimal" defaultValue={r.amount} onBlur={(e) => setRevenue(r.id, { amount: numIn(e.target.value) ?? 0 })} /></td>
              <td><select value={r.currency} onChange={(e) => setRevenue(r.id, { currency: e.target.value as Currency })}><option>BRL</option><option>USD</option><option>EUR</option></select></td>
              <td><select value={r.status} onChange={(e) => setRevenue(r.id, { status: e.target.value as Revenue["status"] })}><option value="prometido">prometido</option><option value="recebido">recebido</option></select></td>
              <td><input type="date" value={r.date ?? ""} onChange={(e) => setRevenue(r.id, { date: e.target.value })} /></td>
              <td><button className="link danger" onClick={() => setF({ revenues: f.revenues.filter((x) => x.id !== r.id) })}>×</button></td>
            </tr>
          ))}
        </tbody>
      </table>
      <button onClick={() => setF({ revenues: [...f.revenues, { id: `r${Date.now()}`, source: "Nova fonte", kind: "patrocinio", amount: 0, currency: "BRL", status: "prometido" }] })}>Acrescentar receita</button>

      <div className="row-buttons export">
        <button onClick={() => download(`${name}-materiais.csv`, csvBr(bomRows(lines)), "text/csv")}>Lista de materiais (CSV)</button>
        <button onClick={() => { const bytes = xlsx([{ name: "Lista de materiais", rows: bomRows(lines) }, { name: "Orçamento", rows: budgetRows }, { name: "Receitas", rows: revenueRows }]); const url = URL.createObjectURL(new Blob([bytes as BlobPart], { type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" })); Object.assign(document.createElement("a"), { href: url, download: `${name}-financas.xlsx` }).click(); URL.revokeObjectURL(url); }}>Planilha (.xlsx)</button>
        <button onClick={() => sponsorReport(design, validation, lines, b, f.revenues)}>Relatório para patrocinadores (PDF)</button>
      </div>

      <Pareto design={design} library={library} materials={materials} core={core} onApply={onChange} />
    </section>
  );
}

/** Abre o relatório para impressão (o navegador salva em PDF). */
function sponsorReport(d: Design, v: Validation | null, lines: BomLine[], b: ReturnType<typeof budget>, revenues: Revenue[]) {
  const w = window.open("", "_blank");
  if (!w) return alert("Permita janelas pop-up para gerar o relatório.");
  const e = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;");
  const s = v?.summary;
  const sponsors = revenues.filter((r) => r.kind === "patrocinio" || r.kind === "parceria");
  w.document.write(`<!doctype html><html lang="pt-BR"><head><meta charset="utf-8"><title>${e(d.name)} — relatório financeiro</title>
<style>body{font:13px/1.5 system-ui,sans-serif;margin:32px;color:#111}h1{margin:0 0 4px}h2{margin-top:24px;border-bottom:2px solid #f0862e}table{border-collapse:collapse;width:100%}td,th{border-bottom:1px solid #ddd;padding:4px 6px;text-align:left}td.n{text-align:right}.muted{color:#666}</style></head><body>
<h1>${e(d.name)}</h1><p class="muted">Relatório gerado pelo ASTRA em ${new Date().toLocaleDateString(locale())}</p>
${s ? `<h2>O foguete</h2><p>Massa na decolagem ${(s.liftoffMass * 1000).toFixed(0)} g · comprimento ${(s.length * 100).toFixed(0)} cm · apogeu previsto ${s.apogee.toFixed(0)} m · margem estática ${s.staticMargin.toFixed(2)} calibres.</p>` : ""}
<h2>Orçamento</h2><table><tr><th>Subsistema</th><th class="n">Previsto</th><th class="n">Gasto</th></tr>${b.subsystems.map((x) => `<tr><td>${x.label}</td><td class="n">${money(x.planned)}</td><td class="n">${money(x.actual)}</td></tr>`).join("")}<tr><th>Total</th><th class="n">${money(b.planned)}</th><th class="n">${money(b.actual)}</th></tr></table>
<h2>Receitas</h2><p>Recebido ${money(b.revenueReceived)} · prometido ${money(b.revenuePromised)} · saldo ${money(b.balance)} · projetado ${money(b.projected)}</p>
${sponsors.length ? `<h2>Patrocinadores e parceiros</h2><ul>${sponsors.map((r) => `<li>${e(r.source)}</li>`).join("")}</ul>` : ""}
<h2>Principais itens</h2><table><tr><th>Item</th><th class="n">Total</th></tr>${[...lines].sort((a, b2) => b2.totalBrl - a.totalBrl).slice(0, 12).map((l) => `<tr><td>${e(l.name)}</td><td class="n">${money(l.totalBrl)}</td></tr>`).join("")}</table>
</body></html>`);
  w.document.close();
  translateDocument(w.document);
  setTimeout(() => w.print(), 300);
}

interface Knob {
  part: string;
  candidates: string[];
}
interface Variant {
  label: string;
  cost: number;
  apogee: number;
  safety: number;
  mass: number;
  design: Design;
}

/** Explorador custo × apogeu × fator de segurança trocando materiais de até duas peças. */
function Pareto({ design, library, materials, core, onApply }: { design: Design; library: ElecComponent[]; materials: Material[]; core: CoreClient; onApply: (d: Design) => void }) {
  const [knobs, setKnobs] = useState<Knob[]>([]);
  const [busy, setBusy] = useState("");
  const [res, setRes] = useState<Variant[]>([]);
  const prices = financeOf(design).materialPrices;
  const combos = knobs.reduce((n, k) => n * Math.max(1, k.candidates.length), 1);
  const run = async () => {
    const lists = knobs.filter((k) => k.candidates.length);
    let variants: { label: string; d: Design }[] = [{ label: "", d: design }];
    for (const k of lists) {
      variants = variants.flatMap((v) =>
        k.candidates.map((m) => ({
          label: `${v.label}${v.label ? " · " : ""}${design.parts.find((p) => p.id === k.part)?.name}: ${materials.find((x) => x.id === m)?.name ?? m}`,
          // Material novo: a peça passa a ter o custo estimado pelo material.
          d: { ...v.d, parts: v.d.parts.map((p) => (p.id === k.part ? { ...p, material: m, cost: prices[m] !== undefined ? null : (p.cost ?? null) } : p)) },
        })),
      );
    }
    const out: Variant[] = [];
    for (const [i, v] of variants.entries()) {
      setBusy(`${i + 1}/${variants.length}`);
      const val = await core.validate(v.d, true);
      const lines = buildBom(v.d, library, val.summary.partMasses ?? []);
      out.push({ label: v.label || "atual", cost: lines.reduce((a, l) => a + l.totalBrl, 0), apogee: val.summary.apogee, safety: val.summary.minSafetyFactor ?? Number.NaN, mass: val.summary.liftoffMass, design: v.d });
    }
    setRes(out);
    setBusy("");
  };
  const valid = res.filter((r) => Number.isFinite(r.safety));
  const front = new Set(paretoFront(valid));
  const W = 520;
  const H = 260;
  const xs = res.map((r) => r.cost);
  const ys = res.map((r) => r.apogee);
  const pad = (lo: number, hi: number): [number, number] => { const m = 0.08 * (hi - lo || Math.abs(hi) || 1); return [lo - m, hi + m]; };
  const [x0, x1] = pad(Math.min(...xs), Math.max(...xs));
  const [y0, y1] = pad(Math.min(...ys), Math.max(...ys));
  const sx = (x: number) => 40 + ((x - x0) / (x1 - x0 || 1)) * (W - 60);
  const sy = (y: number) => H - 30 - ((y - y0) / (y1 - y0 || 1)) * (H - 50);
  const missingPrice = knobs.flatMap((k) => k.candidates).filter((m) => prices[m] === undefined);
  return (
    <div className="pareto">
      <h3>Custo × desempenho (Pareto)</h3>
      <p className="hint">Escolha até duas peças e os materiais candidatos. Cada combinação é simulada com cargas de voo; a fronteira mostra o que não é pior em custo, apogeu e fator de segurança ao mesmo tempo.</p>
      {[0, 1].map((i) => {
        const k = knobs[i];
        return (
          <div key={i} className="knob">
            <select value={k?.part ?? ""} onChange={(e) => { const ks = [...knobs]; if (e.target.value) ks[i] = { part: e.target.value, candidates: [] }; else ks.splice(i, 1); setKnobs(ks.filter(Boolean)); }}>
              <option value="">{i === 0 ? "peça…" : "segunda peça (opcional)…"}</option>
              {design.parts.filter((p) => !["parachute", "shockCord", "mass"].includes(p.kind.type)).map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
            </select>
            {k && (
              <div className="cands">
                {materials.filter((m) => m.category !== "adesivo").map((m) => (
                  <label key={m.id} className="pcheck-inline">
                    <input type="checkbox" checked={k.candidates.includes(m.id)} onChange={(e) => { const ks = [...knobs]; ks[i] = { ...k, candidates: e.target.checked ? [...k.candidates, m.id] : k.candidates.filter((c) => c !== m.id) }; setKnobs(ks); }} />
                    {m.name}
                  </label>
                ))}
              </div>
            )}
          </div>
        );
      })}
      {missingPrice.length > 0 && <p className="pcheck bad">Sem preço por kg: {[...new Set(missingPrice)].map((m) => materials.find((x) => x.id === m)?.name ?? m).join(", ")}. O custo dessas variantes não muda com o material.</p>}
      <button onClick={() => void run()} disabled={!!busy || !knobs.some((k) => k.candidates.length) || combos > 48}>
        {busy ? `Simulando ${busy}…` : combos > 48 ? "Até 48 combinações" : `Simular ${combos} ${combos === 1 ? "variante" : "variantes"}`}
      </button>
      {res.length > 0 && (
        <>
          <svg viewBox={`0 0 ${W} ${H}`} className="scatter" role="img" aria-label="Custo versus apogeu">
            <line x1={40} y1={H - 30} x2={W - 20} y2={H - 30} className="axis" />
            <line x1={40} y1={20} x2={40} y2={H - 30} className="axis" />
            <text x={W / 2} y={H - 6} className="lbl">custo (R$) →</text>
            <text x={6} y={14} className="lbl">apogeu (m)</text>
            {res.map((r) => (
              <circle key={r.label} cx={sx(r.cost)} cy={sy(r.apogee)} r={front.has(r) ? 7 : 4} className={front.has(r) ? "front" : "dom"} style={{ opacity: Number.isFinite(r.safety) ? Math.min(1, 0.35 + r.safety / 4) : 0.3 }}>
                <title>{`${r.label}\nR$ ${r.cost.toFixed(2)} · ${r.apogee.toFixed(0)} m · fator ${Number.isFinite(r.safety) ? r.safety.toFixed(2) : "—"}`}</title>
              </circle>
            ))}
          </svg>
          <table className="bom">
            <thead><tr><th>Variante</th><th>Custo</th><th>Apogeu</th><th>Massa</th><th>Fator de segurança</th><th /></tr></thead>
            <tbody>
              {[...res].sort((a, b) => a.cost - b.cost).map((r) => (
                <tr key={r.label} className={front.has(r) ? "front" : ""}>
                  <td>{front.has(r) ? "★ " : ""}{r.label}</td>
                  <td className="num">{money(r.cost)}</td>
                  <td className="num">{r.apogee.toFixed(0)} m</td>
                  <td className="num">{(r.mass * 1000).toFixed(0)} g</td>
                  <td className="num">{Number.isFinite(r.safety) ? r.safety.toFixed(2) : "—"}</td>
                  <td><button className="link" onClick={() => onApply(r.design)}>aplicar</button></td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
    </div>
  );
}
