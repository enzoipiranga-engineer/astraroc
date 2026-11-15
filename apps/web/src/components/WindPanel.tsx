import { useState } from "react";
import type { Design, WindLayer } from "@astra/core-js";
import { fetchWind, parseWindCsv, powerLawProfile } from "../geo";
import { fmt } from "../units";
import { NumberField } from "./Field";

/** Vento: perfil por altura (manual, lei de potência, CSV ou previsão real) e rajadas. */
export function WindPanel({ design, onChange }: { design: Design; onChange: (d: Design) => void }) {
  const w = design.launch.wind;
  const set = (patch: Partial<typeof w>) => onChange({ ...design, launch: { ...design.launch, wind: { ...w, ...patch } } });
  const setLayers = (layers: WindLayer[], source: string) => set({ layers, source });
  const [gen, setGen] = useState({ v: 5, h: 10, alpha: 1 / 7, from: 90, top: 1500 });
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  const real = async () => {
    const u = design.launch.utc;
    if (!u) return setMsg("Defina a data e a hora do lançamento (UTC) primeiro.");
    setBusy(true);
    setMsg(null);
    try {
      const when = new Date(u);
      const layers = await fetchWind(design.launch.latDeg, design.launch.lonDeg, when, design.launch.altitude);
      setLayers(layers, `Open-Meteo, ${when.toISOString().slice(0, 13)}h UTC, ${design.launch.latDeg.toFixed(3)}, ${design.launch.lonDeg.toFixed(3)}`);
      setMsg(`${layers.length} níveis carregados.`);
    } catch (e) {
      setMsg(`Não foi possível obter o vento: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  };
  const csv = async (f: File | undefined) => {
    if (!f) return;
    try {
      setLayers(parseWindCsv(await f.text()), `CSV ${f.name}`);
      setMsg(null);
    } catch (e) {
      setMsg((e as Error).message);
    }
  };
  const edit = (i: number, k: keyof WindLayer, v: number | null) => {
    if (v === null) return;
    const layers = w.layers.map((l, j) => (j === i ? { ...l, [k]: v } : l));
    set({ layers, source: "manual" });
  };
  const turb = w.turbulence;

  return (
    <section className="panel wind">
      <h2>Vento</h2>
      {w.layers.length === 0 ? <p className="hint">Calmaria (sem vento).</p> : <p className="hint">{w.source ?? "manual"}</p>}
      {w.layers.length > 0 && (
        <table className="wind-table">
          <thead>
            <tr><th>Altura (m)</th><th>m/s</th><th>De (°)</th><th /></tr>
          </thead>
          <tbody>
            {w.layers.map((l, i) => (
              <tr key={i}>
                <td><Cell value={l.agl} onChange={(v) => edit(i, "agl", v)} /></td>
                <td><Cell value={l.speed} onChange={(v) => edit(i, "speed", v)} /></td>
                <td><Cell value={l.fromDeg} onChange={(v) => edit(i, "fromDeg", v)} /></td>
                <td><button className="x" aria-label="Remover camada" onClick={() => set({ layers: w.layers.filter((_, j) => j !== i), source: "manual" })}>×</button></td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      <div className="row-buttons">
        <button onClick={() => set({ layers: [...w.layers, { agl: (w.layers.at(-1)?.agl ?? -100) + 100, speed: w.layers.at(-1)?.speed ?? 3, fromDeg: w.layers.at(-1)?.fromDeg ?? 90 }], source: "manual" })}>+ camada</button>
        <button onClick={() => set({ layers: [], turbulence: null, source: null })}>Calmaria</button>
        <label className="button">CSV<input type="file" accept=".csv,.txt" hidden onChange={(e) => void csv(e.target.files?.[0])} /></label>
        <button onClick={() => void real()} disabled={busy}>{busy ? "Buscando…" : "Vento real (Open-Meteo)"}</button>
      </div>
      {msg && <p className="hint">{msg}</p>}
      <details>
        <summary>Gerar perfil (lei de potência)</summary>
        <div className="row">
          <NumberField label="Vento a 10 m" unit="m" value={gen.v} help="m/s na altura de referência" onChange={(v) => v !== null && setGen({ ...gen, v })} />
          <NumberField label="Vem de" unit="deg" value={gen.from} onChange={(v) => v !== null && setGen({ ...gen, from: v })} />
        </div>
        <div className="row">
          <NumberField label="Expoente α" unit="ratio" value={gen.alpha} help="≈ 0,14 campo aberto; 0,25 a 0,4 terreno com obstáculos." onChange={(v) => v !== null && setGen({ ...gen, alpha: v })} />
          <NumberField label="Até" unit="m" value={gen.top} onChange={(v) => v !== null && setGen({ ...gen, top: v })} />
        </div>
        <button onClick={() => setLayers(powerLawProfile(gen.v, gen.h, gen.alpha, gen.from, gen.top), `lei de potência: ${fmt(gen.v, 1)} m/s a 10 m, α = ${fmt(gen.alpha, 2)}`)}>Gerar</button>
      </details>
      <label className="field check">
        <input type="checkbox" checked={!!turb} onChange={(e) => set({ turbulence: e.target.checked ? { sigma: 1, length: 300, seed: 1 } : null })} />
        <span>Rajadas (turbulência de Dryden)</span>
      </label>
      {turb && (
        <div className="row">
          <NumberField label="Intensidade σ" unit="m" value={turb.sigma} help="Desvio-padrão das rajadas, m/s. Leve ≈ 0,5 a 1; moderada ≈ 2 a 3." onChange={(v) => v !== null && set({ turbulence: { ...turb, sigma: v } })} />
          <NumberField label="Semente" unit="count" value={turb.seed} help="Mesma semente, mesmas rajadas." onChange={(v) => v !== null && set({ turbulence: { ...turb, seed: Math.max(0, Math.round(v)) } })} />
        </div>
      )}
    </section>
  );
}

function Cell({ value, onChange }: { value: number; onChange: (v: number | null) => void }) {
  return (
    <input
      className="cell"
      inputMode="decimal"
      defaultValue={+value.toFixed(2)}
      key={value}
      onBlur={(e) => {
        const v = Number(e.target.value.replace(",", "."));
        onChange(Number.isFinite(v) ? v : null);
      }}
    />
  );
}
