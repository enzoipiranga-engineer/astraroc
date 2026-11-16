import { useState } from "react";
import { CoreClient, type Design, type Dispersion, type McRun, type McSummary, type SimOutput, type Validation } from "@astra/core-js";
import { Chart } from "./Chart";
import { parseTacview } from "../tacview";
import { locale, translateDocument } from "../i18n";

interface Props {
  design: Design;
  core: CoreClient;
  onChange: (d: Design) => void;
  /** Cria um cliente com worker próprio (Monte Carlo em paralelo). */
  spawn: () => { client: CoreClient; stop: () => void };
}

const DEFAULT_DISP: Dispersion = { thrust: 0.03, cd: 0.05, mass: 0.02, windSpeed: 0.2, windDirDeg: 15, elevationDeg: 0.5, azimuthDeg: 2 };
const DISP_FIELDS: { key: keyof Dispersion; label: string; pct: boolean }[] = [
  { key: "thrust", label: "Impulso (σ, %)", pct: true },
  { key: "cd", label: "Cd (σ, %)", pct: true },
  { key: "mass", label: "Massa (σ, %)", pct: true },
  { key: "windSpeed", label: "Vento (σ, %)", pct: true },
  { key: "windDirDeg", label: "Direção do vento (σ, °)", pct: false },
  { key: "elevationDeg", label: "Elevação do trilho (σ, °)", pct: false },
  { key: "azimuthDeg", label: "Azimute do trilho (σ, °)", pct: false },
];
const f = (v: number | null | undefined, d = 0) => (v === null || v === undefined || !Number.isFinite(v) ? "—" : v.toLocaleString(locale(), { minimumFractionDigits: d, maximumFractionDigits: d }));

export function AnalysisPanel({ design, core, onChange, spawn }: Props) {
  return (
    <section className="panel analysis">
      <h2>
        Análises <small>Monte Carlo, sensibilidade, otimização e calibração com telemetria</small>
      </h2>
      <MonteCarlo design={design} core={core} spawn={spawn} />
      <Optimizer design={design} core={core} onChange={onChange} />
      <Calibration design={design} core={core} onChange={onChange} />
    </section>
  );
}

function MonteCarlo({ design, core, spawn }: { design: Design; core: CoreClient; spawn: Props["spawn"] }) {
  const [disp, setDisp] = useState(DEFAULT_DISP);
  const [n, setN] = useState(100);
  const [progress, setProgress] = useState<number | null>(null);
  const [runs, setRuns] = useState<McRun[]>([]);
  const [sum, setSum] = useState<McSummary | null>(null);
  const [sens, setSens] = useState<{ name: string; apogee: number; drift: number }[]>([]);
  const [ms, setMs] = useState(0);
  const run = async () => {
    const threads = Math.max(1, Math.min(8, (navigator.hardwareConcurrency || 4) - 1));
    const pool = Array.from({ length: threads }, () => spawn());
    const chunk = 5;
    const jobs: [number, number][] = [];
    for (let s = 0; s < n; s += chunk) jobs.push([s, Math.min(chunk, n - s)]);
    const all: McRun[] = [];
    const t0 = performance.now();
    setProgress(0);
    setSum(null);
    try {
      await Promise.all(
        pool.map(async (w) => {
          for (let job = jobs.shift(); job; job = jobs.shift()) {
            const r = await w.client.analysis<McRun[]>("monteCarlo", { design, dispersion: disp, seed: 2026, start: job[0], count: job[1] });
            all.push(...r);
            setProgress(all.length / n);
          }
        }),
      );
      all.sort((a, b) => a.index - b.index);
      setRuns(all);
      setSum(await core.analysis<McSummary>("summary", { design, runs: all }));
      setMs(performance.now() - t0);
      setSens(await core.analysis("sensitivity", { design, dispersion: disp }));
    } catch (e) {
      alert(`Falha no Monte Carlo: ${(e as Error).message}`);
    } finally {
      pool.forEach((w) => w.stop());
      setProgress(null);
    }
  };
  const maxSens = Math.max(1e-9, ...sens.map((s) => Math.abs(s.apogee)));
  return (
    <div className="mc">
      <h3>Monte Carlo</h3>
      <div className="fin-controls">
        <label className="field"><span>Rodadas</span><input type="number" min={10} max={2000} value={n} onChange={(e) => setN(Math.max(10, Math.min(2000, Number(e.target.value) || 10)))} /></label>
        {DISP_FIELDS.map((d) => (
          <label key={d.key} className="field">
            <span>{d.label}</span>
            <input type="number" step="any" value={+(disp[d.key] * (d.pct ? 100 : 1)).toPrecision(4)} onChange={(e) => setDisp({ ...disp, [d.key]: Number(e.target.value) / (d.pct ? 100 : 1) })} />
          </label>
        ))}
        <button className="launch small" onClick={() => void run()} disabled={progress !== null}>
          {progress !== null ? `Rodando… ${Math.round(progress * 100)} %` : "RODAR MONTE CARLO"}
        </button>
      </div>
      {sum && (
        <>
          <div className="cards">
            <div className="card"><b>{f(sum.apogee[0])} ± {f(sum.apogee[1])} m</b><span>apogeu (média ± 1σ)</span></div>
            <div className="card"><b>{f(sum.apogee[2])} – {f(sum.apogee[4])} m</b><span>90 % das rodadas (P5–P95)</span></div>
            <div className="card"><b>{f(sum.ellipse.axes95[0])} × {f(sum.ellipse.axes95[1])} m</b><span>elipse de 95 % dos pousos</span></div>
            {sum.pLandingZone !== null && <div className="card"><b>{f(100 * sum.pLandingZone, 1)} %</b><span>pousam na zona de pouso</span></div>}
            {sum.pKeepOut !== null && <div className={`card ${sum.pKeepOut > 0 ? "bad" : ""}`}><b>{f(100 * sum.pKeepOut, 1)} %</b><span>com peça em zona proibida</span></div>}
            <div className={`card ${sum.failureRate > 0 ? "bad" : ""}`}><b>{f(100 * sum.failureRate, 1)} %</b><span>com falha estrutural ou de recuperação</span></div>
            <div className="card"><b>{f(ms / 1000, 1)} s</b><span>{sum.n} rodadas em paralelo</span></div>
          </div>
          <div className="mc-grid">
            <Histogram sum={sum} />
            <Dispersal design={design} runs={runs} sum={sum} />
            <div>
              <h4>Sensibilidade (±1σ, um de cada vez)</h4>
              <table className="tornado">
                <tbody>
                  {sens.map((s) => (
                    <tr key={s.name}>
                      <td>{s.name}</td>
                      <td className="bar-cell"><div className={`bar ${s.apogee < 0 ? "neg" : ""}`} style={{ width: `${(50 * Math.abs(s.apogee)) / maxSens}%`, marginLeft: s.apogee < 0 ? `${50 - (50 * Math.abs(s.apogee)) / maxSens}%` : "50%" }} /></td>
                      <td className="num">{s.apogee > 0 ? "+" : ""}{f(s.apogee)} m</td>
                      <td className="num">{f(s.drift)} m</td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <p className="hint">Colunas: Δ apogeu e Δ distância de pouso de −1σ a +1σ.</p>
            </div>
          </div>
        </>
      )}
    </div>
  );
}

function Histogram({ sum }: { sum: McSummary }) {
  const max = Math.max(...sum.histogram.map((h) => h[1]), 1);
  const W = 300;
  const H = 160;
  const bw = W / sum.histogram.length;
  return (
    <div>
      <h4>Distribuição do apogeu</h4>
      <svg viewBox={`0 0 ${W} ${H + 20}`} className="hist">
        {sum.histogram.map((h, i) => <rect key={i} x={i * bw + 1} y={H - (h[1] / max) * H} width={bw - 2} height={(h[1] / max) * H} />)}
        <text x={0} y={H + 14} className="lbl">{f(sum.histogram[0]?.[0])} m</text>
        <text x={W} y={H + 14} className="lbl" textAnchor="end">{f((sum.histogram.at(-1)?.[0] ?? 0) + (sum.histogram[1]?.[0] ?? 0) - (sum.histogram[0]?.[0] ?? 0))} m</text>
      </svg>
    </div>
  );
}

function Dispersal({ design, runs, sum }: { design: Design; runs: McRun[]; sum: McSummary }) {
  const pts = runs.flatMap((r) => r.landings);
  const re = 6378137;
  const lat0 = (design.launch.latDeg * Math.PI) / 180;
  const zones = design.zones.map((z) => ({ ...z, n: ((z.latDeg - design.launch.latDeg) * Math.PI) / 180 * re, e: ((z.lonDeg - design.launch.lonDeg) * Math.PI) / 180 * re * Math.cos(lat0) }));
  const ext = Math.max(50, ...pts.map((p) => Math.max(Math.abs(p[0]), Math.abs(p[1]))), ...zones.map((z) => Math.max(Math.abs(z.n), Math.abs(z.e)) + z.radius), sum.ellipse.axes95[0] + Math.hypot(...sum.ellipse.center)) * 1.1;
  const s = 140 / ext;
  const el = sum.ellipse;
  return (
    <div>
      <h4>Pousos (norte para cima)</h4>
      <svg viewBox="-150 -150 300 300" className="disp">
        <line x1={-150} y1={0} x2={150} y2={0} className="axis" />
        <line x1={0} y1={-150} x2={0} y2={150} className="axis" />
        {zones.map((z) => <circle key={z.name} cx={z.e * s} cy={-z.n * s} r={z.radius * s} className={z.kind === "keepOut" ? "keep" : "land"} />)}
        {pts.map((p, i) => <circle key={i} cx={p[1] * s} cy={-p[0] * s} r={1.8} className="pt" />)}
        {/* Elipses: eixo maior no azimute (a partir do norte). */}
        {[el.axes1sigma, el.axes95].map((ax, k) => (
          <ellipse key={k} cx={el.center[1] * s} cy={-el.center[0] * s} rx={ax[1] * s} ry={ax[0] * s} transform={`rotate(${el.azimuthDeg} ${el.center[1] * s} ${-el.center[0] * s})`} className={k ? "e95" : "e1"} />
        ))}
        <circle r={3} className="pad" />
        <text x={-146} y={-138} className="lbl">{f(ext)} m</text>
      </svg>
    </div>
  );
}

function numericFields(design: Design, partId: string): string[] {
  const p = design.parts.find((x) => x.id === partId);
  if (!p) return [];
  return ["massOverride", ...Object.entries(p.kind).filter(([, v]) => typeof v === "number").map(([k]) => k)];
}
const FIELD_LABEL: Record<string, string> = { massOverride: "massa medida (kg)", mass: "massa (kg)", length: "comprimento (m)", span: "envergadura (m)", rootChord: "corda da raiz (m)", tipChord: "corda da ponta (m)", sweep: "enflechamento (m)", diameter: "diâmetro (m)", wall: "parede (m)", thickness: "espessura (m)", count: "quantidade" };

function Optimizer({ design, core, onChange }: { design: Design; core: CoreClient; onChange: (d: Design) => void }) {
  const [part, setPart] = useState(design.parts.find((p) => p.kind.type === "mass")?.id ?? design.parts[0]?.id ?? "");
  const [field, setField] = useState("massOverride");
  const [range, setRange] = useState({ min: 0.01, max: 0.5, target: 500 });
  const [res, setRes] = useState<{ value: number; apogee: number; reached: boolean; design: Design; evaluations: [number, number][] } | null>(null);
  const [busy, setBusy] = useState(false);
  const fields = numericFields(design, part);
  const run = async () => {
    setBusy(true);
    try {
      setRes(await core.analysis("optimize", { design, knob: { part, field, min: range.min, max: range.max }, target: range.target }));
    } catch (e) {
      alert((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="opt">
      <h3>Otimizador: apogeu-alvo</h3>
      <div className="fin-controls">
        <label className="field"><span>Peça</span><select value={part} onChange={(e) => { setPart(e.target.value); setField("massOverride"); }}>{design.parts.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}</select></label>
        <label className="field"><span>Parâmetro</span><select value={field} onChange={(e) => setField(e.target.value)}>{fields.map((k) => <option key={k} value={k}>{FIELD_LABEL[k] ?? k}</option>)}</select></label>
        <label className="field"><span>Mínimo</span><input type="number" step="any" value={range.min} onChange={(e) => setRange({ ...range, min: Number(e.target.value) })} /></label>
        <label className="field"><span>Máximo</span><input type="number" step="any" value={range.max} onChange={(e) => setRange({ ...range, max: Number(e.target.value) })} /></label>
        <label className="field"><span>Apogeu-alvo (m)</span><input type="number" value={range.target} onChange={(e) => setRange({ ...range, target: Number(e.target.value) })} /></label>
        <button onClick={() => void run()} disabled={busy}>{busy ? "Otimizando…" : "Otimizar"}</button>
      </div>
      {res && (
        <p className={`pcheck ${res.reached ? "ok" : "bad"}`}>
          {FIELD_LABEL[field] ?? field} = <b>{f(res.value, 4)}</b> → apogeu {f(res.apogee, 1)} m {res.reached ? "(no alvo)" : "(o alvo não está no intervalo)"} · {res.evaluations.length} simulações{" "}
          <button className="link" onClick={() => onChange(res.design)}>aplicar</button>
        </p>
      )}
    </div>
  );
}

/** Lê CSV de telemetria: colunas tempo (s) e altura (m), separadas por vírgula, ponto e vírgula ou tab. */
export function parseTelemetry(text: string): [number, number][] {
  // CSV do Tacview: tempo e altitude acima do mar.
  const tac = parseTacview(text);
  if (tac && tac.length >= 3) text = tac.map((p) => `${p.t},${p.alt}`).join("\n");
  const rows: [number, number][] = [];
  for (const line of text.split(/\r?\n/)) {
    const cells = line.split(/[;,\t]/).map((c) => Number(c.trim()));
    if (cells.length >= 2 && Number.isFinite(cells[0]) && Number.isFinite(cells[1])) rows.push([cells[0]!, cells[1]!]);
  }
  if (rows.length < 3) return rows;
  // Altura relativa ao primeiro ponto (altímetros gravam a altitude do campo).
  const h0 = rows[0]![1];
  const t0 = rows.find((r) => r[1] - h0 > 2)?.[0] ?? rows[0]![0];
  return rows.map(([t, h]) => [t - t0, h - h0] as [number, number]).filter((r) => r[0] >= 0);
}

function Calibration({ design, core, onChange }: { design: Design; core: CoreClient; onChange: (d: Design) => void }) {
  const [data, setData] = useState<[number, number][]>([]);
  const [fitThrust, setFitThrust] = useState(false);
  const [res, setRes] = useState<{ cd: number; thrust: number; rmsBefore: number; rmsAfter: number; apogee: [number, number, number]; fitted: [number, number][] } | null>(null);
  const [busy, setBusy] = useState(false);
  const load = async (file: File | undefined) => {
    if (!file) return;
    const d = parseTelemetry(await file.text());
    if (d.length < 3) return alert("Não encontrei colunas tempo e altura no arquivo.");
    setData(d);
    setRes(null);
  };
  const run = async () => {
    setBusy(true);
    try {
      setRes(await core.analysis("calibrate", { design, data: data.filter((p) => p[0] < 600), fitThrust }));
    } catch (e) {
      alert((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="calib">
      <h3>Calibração com telemetria</h3>
      <p className="hint">Carregue o registro do altímetro (CSV com tempo em s e altura em m, ou CSV do Tacview). O ASTRA ajusta o arrasto (e, se pedir, o impulso) para a simulação bater com o voo real.</p>
      <div className="fin-controls">
        <label className="button">Carregar CSV<input type="file" accept=".csv,.txt" hidden onChange={(e) => void load(e.target.files?.[0])} /></label>
        <label className="field check"><input type="checkbox" checked={fitThrust} onChange={(e) => setFitThrust(e.target.checked)} /><span>Ajustar também o impulso</span></label>
        <button onClick={() => void run()} disabled={busy || data.length < 3}>{busy ? "Ajustando…" : `Calibrar (${data.length} pontos)`}</button>
      </div>
      {res && (
        <>
          <p className="pcheck ok">
            Cd × {f(res.cd, 3)}{fitThrust ? ` · impulso × ${f(res.thrust, 3)}` : ""} · erro RMS {f(res.rmsBefore, 1)} m → {f(res.rmsAfter, 1)} m · apogeu medido {f(res.apogee[0])} m, simulado {f(res.apogee[1])} → {f(res.apogee[2])} m{" "}
            <button className="link" onClick={() => onChange({ ...design, scales: { cd: res.cd, thrust: res.thrust, mass: design.scales?.mass ?? 1 } })}>aplicar ao projeto</button>
          </p>
          <Chart title="Altura: medida × simulada calibrada" unit="m" cursorSync="cal" series={[{ label: "medida", color: "#f0c36d", t: data.map((p) => p[0]), y: data.map((p) => p[1]) }, { label: "calibrada", color: "#5eb1ff", t: res.fitted.map((p) => p[0]), y: res.fitted.map((p) => p[1]) }]} />
        </>
      )}
      {design.scales && (design.scales.cd !== 1 || design.scales.thrust !== 1 || design.scales.mass !== 1) && (
        <p className="hint">
          Multiplicadores em uso: Cd × {f(design.scales.cd, 3)}, impulso × {f(design.scales.thrust, 3)}, massa × {f(design.scales.mass, 3)}{" "}
          <button className="link" onClick={() => onChange({ ...design, scales: { cd: 1, thrust: 1, mass: 1 } })}>voltar ao nominal</button>
        </p>
      )}
    </div>
  );
}

/** Relatório de voo pronto para imprimir (o navegador salva em PDF). */
export function flightReport(design: Design, v: Validation | null, out: SimOutput) {
  const w = window.open("", "_blank");
  if (!w) return alert("Permita janelas pop-up para gerar o relatório.");
  const e = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;");
  const s = v?.summary;
  const charts = [...document.querySelectorAll<HTMLCanvasElement>(".results canvas")].slice(0, 4).map((c) => c.toDataURL("image/png"));
  w.document.write(`<!doctype html><html lang="pt-BR"><head><meta charset="utf-8"><title>${e(design.name)} — relatório de voo</title>
<style>body{font:13px/1.5 system-ui,sans-serif;margin:32px;color:#111}h2{border-bottom:2px solid #f0862e;margin-top:22px}table{border-collapse:collapse;width:100%}td,th{border-bottom:1px solid #ddd;padding:3px 6px;text-align:left}img{max-width:48%;margin:1%;background:#0b1018}.muted{color:#666}</style></head><body>
<h1>${e(design.name)}</h1><p class="muted">Relatório de voo simulado — ASTRA, ${new Date().toLocaleString(locale())}. Local ${design.launch.latDeg.toFixed(5)}, ${design.launch.lonDeg.toFixed(5)}.</p>
${s ? `<h2>Projeto</h2><table><tr><td>Massa na decolagem</td><td>${(s.liftoffMass * 1000).toFixed(0)} g</td></tr><tr><td>Comprimento × diâmetro</td><td>${(s.length * 100).toFixed(1)} × ${(s.diameter * 1000).toFixed(0)} mm</td></tr><tr><td>Margem estática</td><td>${s.staticMargin.toFixed(2)} cal</td></tr><tr><td>Empuxo/peso</td><td>${s.thrustToWeight.toFixed(1)}</td></tr></table>` : ""}
<h2>Voo</h2><table><tr><th>Corpo</th><th>Modelo</th><th>Destino</th></tr>${out.bodies.map((b) => `<tr><td>${e(b.name)}</td><td>${e(b.models.join(" → "))}</td><td>${e(b.fate)}</td></tr>`).join("")}</table>
<h2>Acontecimentos</h2><table>${out.events.map((ev) => `<tr><td>${ev.t.toFixed(2)} s</td><td>${e(ev.message)}</td></tr>`).join("")}</table>
${charts.length ? `<h2>Gráficos</h2>${charts.map((c) => `<img src="${c}">`).join("")}` : ""}
${v?.issues.length ? `<h2>Avisos</h2><ul>${v.issues.map((i) => `<li>${e(i.message)}</li>`).join("")}</ul>` : ""}
${out.warnings.length ? `<ul>${out.warnings.map((x) => `<li>${e(x)}</li>`).join("")}</ul>` : ""}
</body></html>`);
  w.document.close();
  translateDocument(w.document);
  setTimeout(() => w.print(), 400);
}
