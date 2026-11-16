import { useEffect, useState } from "react";
import type { CoreClient } from "@astra/core-js";
import { Chart } from "./Chart";

type Tool = "maneuver" | "interplanetary" | "lambert" | "lagrange" | "entry" | "landing";
const TOOLS: { id: Tool; label: string }[] = [
  { id: "maneuver", label: "Manobras" },
  { id: "interplanetary", label: "Interplanetário" },
  { id: "lambert", label: "Lambert" },
  { id: "lagrange", label: "Lagrange" },
  { id: "entry", label: "Reentrada" },
  { id: "landing", label: "Pouso propulsivo" },
];

interface BodyInfo {
  body: { id: string; name: string; gm: number; radius: number; atmosphere: unknown; parent: string | null; helio: unknown };
  escapeSpeed: number;
  surfaceGravity: number;
  soi: number | null;
}

const f = (v: number, d = 0) => (Number.isFinite(v) ? v.toLocaleString("pt-BR", { minimumFractionDigits: d, maximumFractionDigits: d }) : "—");
const J2000 = Date.UTC(2000, 0, 1, 12);
const toDate = (s: number) => new Date(J2000 + s * 1000).toLocaleDateString("pt-BR", { year: "numeric", month: "short", day: "numeric" });

/** Campo numérico simples (valor na unidade mostrada). */
function N({ label, value, onChange, step }: { label: string; value: number; onChange: (v: number) => void; step?: number }) {
  return (
    <label className="field">
      <span>{label}</span>
      <input type="number" step={step ?? "any"} value={value} onChange={(e) => onChange(Number(e.target.value))} />
    </label>
  );
}

function BodySelect({ label, value, bodies, onChange, filter }: { label: string; value: string; bodies: BodyInfo[]; onChange: (v: string) => void; filter?: (b: BodyInfo) => boolean }) {
  return (
    <label className="field">
      <span>{label}</span>
      <select value={value} onChange={(e) => onChange(e.target.value)}>
        {bodies.filter(filter ?? (() => true)).map((b) => <option key={b.body.id} value={b.body.id}>{b.body.name}</option>)}
      </select>
    </label>
  );
}

export function SpaceView({ core }: { core: CoreClient }) {
  const [tool, setTool] = useState<Tool>("maneuver");
  const [bodies, setBodies] = useState<BodyInfo[]>([]);
  useEffect(() => void core.space<BodyInfo[]>("bodies").then(setBodies), [core]);
  return (
    <div className="space">
      <section className="panel">
        <h2>Espaço <small>órbitas, transferências, reentrada e pouso em outros corpos</small></h2>
        <div className="modes sub" role="tablist">
          {TOOLS.map((t) => <button key={t.id} role="tab" aria-selected={tool === t.id} className={tool === t.id ? "on" : ""} onClick={() => setTool(t.id)}>{t.label}</button>)}
        </div>
      </section>
      {bodies.length > 0 && (
        <>
          {tool === "maneuver" && <Maneuver core={core} bodies={bodies} />}
          {tool === "interplanetary" && <Interplanetary core={core} bodies={bodies} />}
          {tool === "lambert" && <Lambert core={core} bodies={bodies} />}
          {tool === "lagrange" && <Lagrange core={core} bodies={bodies} />}
          {tool === "entry" && <Entry core={core} bodies={bodies} />}
          {tool === "landing" && <Landing core={core} bodies={bodies} />}
        </>
      )}
      <p className="note">Dois corpos (Kepler) e cônicas combinadas; janelas no modelo de órbitas circulares e coplanares (erro de semanas). Ver docs/MODELOS.md.</p>
    </div>
  );
}

function useTool<T>(core: CoreClient, tool: string, input: unknown): { out: T | null; err: string } {
  const [out, setOut] = useState<T | null>(null);
  const [err, setErr] = useState("");
  const key = JSON.stringify(input);
  useEffect(() => {
    let live = true;
    core.space<T>(tool, JSON.parse(key)).then((o) => { if (live) { setOut(o); setErr(""); } }, (e: Error) => live && setErr(e.message));
    return () => { live = false; };
  }, [core, tool, key]);
  return { out, err };
}

interface Tr { burns: number[]; total: number; tof: number }

function Maneuver({ core, bodies }: { core: CoreClient; bodies: BodyInfo[] }) {
  const [inp, setInp] = useState({ body: "terra", h1: 300, h2: 35_786, hb: 0, diDeg: 28.5 });
  const { out, err } = useTool<{ hohmann: Tr; bielliptic: Tr | null; planeChangeLow: number; planeChangeHigh: number; v1: number; v2: number; period1: number; period2: number }>(core, "maneuver", { body: inp.body, h1: inp.h1 * 1e3, h2: inp.h2 * 1e3, hb: inp.hb > 0 ? inp.hb * 1e3 : null, diDeg: inp.diDeg });
  return (
    <section className="panel">
      <div className="orbital-form">
        <BodySelect label="Corpo central" value={inp.body} bodies={bodies} onChange={(body) => setInp({ ...inp, body })} />
        <N label="Altitude inicial (km)" value={inp.h1} onChange={(h1) => setInp({ ...inp, h1 })} />
        <N label="Altitude final (km)" value={inp.h2} onChange={(h2) => setInp({ ...inp, h2 })} />
        <N label="Apoapside da bi-elíptica (km, 0 = sem)" value={inp.hb} onChange={(hb) => setInp({ ...inp, hb })} />
        <N label="Mudança de inclinação (°)" value={inp.diDeg} onChange={(diDeg) => setInp({ ...inp, diDeg })} />
      </div>
      {err && <p className="bad">{err}</p>}
      {out && (
        <table className="bodies">
          <tbody>
            <tr><td>Velocidade circular inicial / final</td><td className="num">{f(out.v1)} / {f(out.v2)} m/s</td></tr>
            <tr><td>Período inicial / final</td><td className="num">{f(out.period1 / 60, 1)} / {f(out.period2 / 60, 1)} min</td></tr>
            <tr><td>Hohmann: Δv₁ + Δv₂</td><td className="num">{f(out.hohmann.burns[0]!)} + {f(out.hohmann.burns[1]!)} = <b>{f(out.hohmann.total)} m/s</b> em {f(out.hohmann.tof / 3600, 2)} h</td></tr>
            {out.bielliptic && <tr><td>Bi-elíptica: três queimas</td><td className="num">{out.bielliptic.burns.map((b) => f(b)).join(" + ")} = <b>{f(out.bielliptic.total)} m/s</b> em {f(out.bielliptic.tof / 3600, 1)} h</td></tr>}
            <tr><td>Mudança de plano pura na órbita baixa / alta</td><td className="num">{f(out.planeChangeLow)} / {f(out.planeChangeHigh)} m/s</td></tr>
          </tbody>
        </table>
      )}
      <p className="hint">Mudar o plano na órbita alta (mais lenta) custa bem menos: por isso a inclinação de GEO é tirada no apogeu da transferência.</p>
    </section>
  );
}

interface Ip { vInfDep: number; c3: number; dvDepart: number; vInfArr: number; dvCapture: number; entrySpeed: number | null; tof: number; phase: number; synodic: number; windows: number[] }

function Interplanetary({ core, bodies }: { core: CoreClient; bodies: BodyInfo[] }) {
  const [inp, setInp] = useState({ from: "terra", to: "marte", hDep: 300, hArr: 400 });
  const after = (Date.now() - J2000) / 1000;
  const { out, err } = useTool<Ip>(core, "interplanetary", { from: inp.from, to: inp.to, hDep: inp.hDep * 1e3, hArr: inp.hArr * 1e3, after: Math.floor(after / 86400) * 86400 });
  const planets = (b: BodyInfo) => b.body.helio !== null;
  const ra = bodies.find((b) => b.body.id === inp.from);
  const rb = bodies.find((b) => b.body.id === inp.to);
  return (
    <section className="panel">
      <div className="orbital-form">
        <BodySelect label="Partida" value={inp.from} bodies={bodies} filter={planets} onChange={(from) => setInp({ ...inp, from })} />
        <BodySelect label="Destino" value={inp.to} bodies={bodies} filter={planets} onChange={(to) => setInp({ ...inp, to })} />
        <N label="Órbita de estacionamento (km)" value={inp.hDep} onChange={(hDep) => setInp({ ...inp, hDep })} />
        <N label="Órbita de chegada (km)" value={inp.hArr} onChange={(hArr) => setInp({ ...inp, hArr })} />
      </div>
      {err && <p className="bad">{err}</p>}
      {out && ra && rb && (
        <div className="space-grid">
          <table className="bodies">
            <tbody>
              <tr><td>v∞ na partida · C3</td><td className="num">{f(out.vInfDep)} m/s · {f(out.c3 / 1e6, 1)} km²/s²</td></tr>
              <tr><td>Δv saindo da órbita de {inp.hDep} km</td><td className="num"><b>{f(out.dvDepart)} m/s</b></td></tr>
              <tr><td>v∞ na chegada</td><td className="num">{f(out.vInfArr)} m/s</td></tr>
              <tr><td>Δv de captura em {inp.hArr} km</td><td className="num"><b>{f(out.dvCapture)} m/s</b></td></tr>
              {out.entrySpeed !== null && <tr><td>Ou: velocidade de entrada direta na atmosfera</td><td className="num">{f(out.entrySpeed)} m/s</td></tr>}
              <tr><td>Tempo de voo</td><td className="num">{f(out.tof / 86400)} dias</td></tr>
              <tr><td>Fase na partida (destino à frente)</td><td className="num">{f((out.phase * 180) / Math.PI, 1)}°</td></tr>
              <tr><td>Período sinódico</td><td className="num">{f(out.synodic / 86400)} dias</td></tr>
              <tr><td>Próximas janelas (aprox.)</td><td className="num">{out.windows.slice(0, 4).map(toDate).join(" · ")}</td></tr>
            </tbody>
          </table>
          <TransferSvg r1={1} r2={rbA(rb) / rbA(ra)} />
        </div>
      )}
    </section>
  );
}
const rbA = (b: BodyInfo) => (b.body.helio as { a: number } | null)?.a ?? 1;

function TransferSvg({ r1, r2 }: { r1: number; r2: number }) {
  const R = Math.max(r1, r2);
  const s = 130 / R;
  const a = ((r1 + r2) / 2) * s;
  const c = a - r1 * s;
  const b = Math.sqrt(Math.max(0, a * a - c * c));
  return (
    <svg viewBox="-150 -150 300 300" className="orbit-svg" role="img" aria-label="Transferência de Hohmann">
      <circle r={6} className="sun" />
      <circle r={r1 * s} className="orb" />
      <circle r={r2 * s} className="orb" />
      <path d={`M ${r1 * s} 0 A ${a} ${b} 0 0 0 ${-r2 * s} 0`} className="xfer" transform={`translate(${r1 < r2 ? 0 : 0} 0)`} />
      <circle cx={r1 * s} cy={0} r={4} className="p1" />
      <circle cx={-r2 * s} cy={0} r={4} className="p2" />
    </svg>
  );
}

function Lambert({ core, bodies }: { core: CoreClient; bodies: BodyInfo[] }) {
  const [inp, setInp] = useState({ body: "terra", r1: [5000, 10000, 2100], r2: [-14600, 2500, 7000], tof: 60, prograde: true });
  const { out, err } = useTool<{ v1: number[]; v2: number[]; elements: { a: number; e: number; i: number } }>(core, "lambert", { body: inp.body, r1: inp.r1.map((x) => x * 1e3), r2: inp.r2.map((x) => x * 1e3), tof: inp.tof * 60, prograde: inp.prograde });
  const vec = (k: "r1" | "r2") => (
    <div className="row3">
      {[0, 1, 2].map((i) => <N key={i} label={`${k} ${"xyz"[i]} (km)`} value={inp[k][i]!} onChange={(v) => setInp({ ...inp, [k]: inp[k].map((x, j) => (j === i ? v : x)) })} />)}
    </div>
  );
  return (
    <section className="panel">
      <div className="orbital-form">
        <BodySelect label="Corpo central" value={inp.body} bodies={bodies} onChange={(body) => setInp({ ...inp, body })} />
        <N label="Tempo de voo (min)" value={inp.tof} onChange={(tof) => setInp({ ...inp, tof })} />
        <label className="field check"><input type="checkbox" checked={inp.prograde} onChange={(e) => setInp({ ...inp, prograde: e.target.checked })} /><span>Prógrada</span></label>
      </div>
      {vec("r1")}
      {vec("r2")}
      {err && <p className="bad">{err}</p>}
      {out && (
        <table className="bodies">
          <tbody>
            <tr><td>v₁ (km/s)</td><td className="num">{out.v1.map((x) => f(x / 1e3, 4)).join(", ")}</td></tr>
            <tr><td>v₂ (km/s)</td><td className="num">{out.v2.map((x) => f(x / 1e3, 4)).join(", ")}</td></tr>
            <tr><td>Órbita de transferência</td><td className="num">a = {f(out.elements.a / 1e3)} km · e = {f(out.elements.e, 4)} · i = {f((out.elements.i * 180) / Math.PI, 2)}°</td></tr>
          </tbody>
        </table>
      )}
      <p className="hint">Valores iniciais: exemplo 5.2 do Curtis (Orbital Mechanics for Engineering Students).</p>
    </section>
  );
}

function Lagrange({ core, bodies }: { core: CoreClient; bodies: BodyInfo[] }) {
  const pairs = [["terra", "lua"], ["sol", "terra"], ["sol", "marte"], ["sol", "jupiter"], ["saturno", "tita"]] as const;
  const [k, setK] = useState(0);
  const [p, s] = pairs[k]!;
  const { out } = useTool<{ mu: number; points: [number, number][]; jacobi: number[] }>(core, "lagrange", { primary: p, secondary: s });
  const name = (id: string) => bodies.find((b) => b.body.id === id)?.body.name ?? id;
  const d = out ? Math.hypot(...out.points[3]!) : 1;
  const sc = 120 / (out ? Math.max(...out.points.map((q) => Math.hypot(...q))) : 1);
  return (
    <section className="panel">
      <label className="field"><span>Sistema</span><select value={k} onChange={(e) => setK(Number(e.target.value))}>{pairs.map(([a, b], i) => <option key={i} value={i}>{name(a)}–{name(b)}</option>)}</select></label>
      {out && (
        <div className="space-grid">
          <table className="bodies">
            <tbody>
              <tr><td>μ = m₂/(m₁+m₂)</td><td className="num">{out.mu.toExponential(4)}</td></tr>
              {out.points.map((q, i) => {
                const fromSec = Math.hypot(q[0] - (1 - out.mu) * d, q[1]);
                return <tr key={i}><td>L{i + 1}</td><td className="num">{f(Math.hypot(q[0] + out.mu * d, q[1]) / 1e3)} km de {name(p)} · {f(fromSec / 1e3)} km de {name(s)}</td></tr>;
              })}
            </tbody>
          </table>
          <svg viewBox="-150 -150 300 300" className="orbit-svg" role="img" aria-label="Pontos de Lagrange">
            <circle cx={-out.mu * d * sc} r={6} className="sun" />
            <circle cx={(1 - out.mu) * d * sc} r={4} className="p2" />
            {out.points.map((q, i) => <g key={i}><circle cx={q[0] * sc} cy={-q[1] * sc} r={3} className="lpt" /><text x={q[0] * sc + 5} y={-q[1] * sc - 5} className="lbl">L{i + 1}</text></g>)}
          </svg>
        </div>
      )}
    </section>
  );
}

interface EntryOut { rows: number[][]; peakG: number; peakGAlt: number; peakHeat: number; heatLoad: number; maxWallTemp: number; ballisticCoefficient: number; allenEggersG: number | null; chuteOpen: number[] | null; chuteShockG: number; impactSpeed: number; downrange: number; duration: number; skipped: boolean; tpsOk: boolean | null; warnings: string[] }

const ENTRY_PRESETS: Record<string, Record<string, unknown>> = {
  "Cápsula, retorno lunar (Terra)": { body: "terra", mass: 5500, diameter: 3.9, cd: 1.3, liftToDrag: 0.3, bankDeg: 45, noseRadius: 4.7, emissivity: 0.85, entryAlt: 122000, entrySpeed: 11000, gammaDeg: -6.5, chute: { diameter: 25, cd: 0.8, deployMach: 0.8, deployAlt: 7000 }, tpsMaxC: 2700 },
  "Cápsula, órbita baixa (Terra)": { body: "terra", mass: 3000, diameter: 2.5, cd: 1.3, liftToDrag: 0.25, bankDeg: 45, noseRadius: 3, emissivity: 0.85, entryAlt: 120000, entrySpeed: 7800, gammaDeg: -1.5, chute: { diameter: 20, cd: 0.8, deployMach: 0.8, deployAlt: 5000 }, tpsMaxC: 1600 },
  "Sonda balística em Marte": { body: "marte", mass: 600, diameter: 2.65, cd: 1.6, liftToDrag: 0, bankDeg: 0, noseRadius: 0.66, emissivity: 0.85, entryAlt: 125000, entrySpeed: 5500, gammaDeg: -14, chute: { diameter: 12, cd: 0.6, deployMach: 2, deployAlt: 12000 }, tpsMaxC: 1600 },
  "Sonda em Titã": { body: "tita", mass: 320, diameter: 2.7, cd: 1.5, liftToDrag: 0, bankDeg: 0, noseRadius: 1.25, emissivity: 0.85, entryAlt: 1270000, entrySpeed: 6000, gammaDeg: -65, chute: { diameter: 8, cd: 0.6, deployMach: 1.5, deployAlt: 180000 }, tpsMaxC: 1500 },
};

function Entry({ core, bodies }: { core: CoreClient; bodies: BodyInfo[] }) {
  const [inp, setInp] = useState<Record<string, unknown>>(ENTRY_PRESETS["Cápsula, retorno lunar (Terra)"]!);
  const { out, err } = useTool<EntryOut>(core, "entry", inp);
  const n = (k: string) => inp[k] as number;
  const set = (k: string) => (v: number) => setInp({ ...inp, [k]: v });
  const t = out?.rows.map((r) => r[0]!) ?? [];
  const col = (i: number, k = 1) => out?.rows.map((r) => r[i]! * k) ?? [];
  return (
    <section className="panel">
      <div className="orbital-form">
        <label className="field"><span>Exemplo</span><select value="" onChange={(e) => e.target.value && setInp(ENTRY_PRESETS[e.target.value]!)}><option value="">escolher…</option>{Object.keys(ENTRY_PRESETS).map((k) => <option key={k}>{k}</option>)}</select></label>
        <BodySelect label="Corpo" value={inp.body as string} bodies={bodies} filter={(b) => b.body.atmosphere !== null} onChange={(body) => setInp({ ...inp, body })} />
        <N label="Massa (kg)" value={n("mass")} onChange={set("mass")} />
        <N label="Diâmetro do escudo (m)" value={n("diameter")} onChange={set("diameter")} />
        <N label="Cd" value={n("cd")} onChange={set("cd")} />
        <N label="L/D" value={n("liftToDrag")} onChange={set("liftToDrag")} step={0.05} />
        <N label="Rolamento (°)" value={n("bankDeg")} onChange={set("bankDeg")} />
        <N label="Raio do nariz (m)" value={n("noseRadius")} onChange={set("noseRadius")} />
        <N label="Velocidade de entrada (m/s)" value={n("entrySpeed")} onChange={set("entrySpeed")} />
        <N label="Ângulo de entrada (°)" value={n("gammaDeg")} onChange={set("gammaDeg")} step={0.1} />
        <N label="Altitude da interface (m)" value={n("entryAlt")} onChange={set("entryAlt")} />
        <N label="Temperatura máx. da proteção (°C)" value={n("tpsMaxC")} onChange={set("tpsMaxC")} />
      </div>
      {err && <p className="bad">{err}</p>}
      {out && (
        <>
          <div className="cards">
            <div className="card"><b>{f(out.peakG, 1)} g</b><span>pico de desaceleração a {f(out.peakGAlt / 1e3)} km{out.allenEggersG !== null ? ` (Allen-Eggers: ${f(out.allenEggersG, 1)} g)` : ""}</span></div>
            <div className="card"><b>{f(out.peakHeat / 1e6, 2)} MW/m²</b><span>pico de calor no nariz</span></div>
            <div className="card"><b>{f(out.heatLoad / 1e6)} MJ/m²</b><span>carga térmica</span></div>
            <div className={`card ${out.tpsOk === false ? "bad" : ""}`}><b>{f(out.maxWallTemp - 273.15)} °C</b><span>parede (equilíbrio radiativo)</span></div>
            <div className="card"><b>{f(out.ballisticCoefficient)} kg/m²</b><span>coeficiente balístico</span></div>
            {out.chuteOpen && <div className="card"><b>{f(out.chuteShockG, 1)} g</b><span>tranco do paraquedas (a {f(out.chuteOpen[1]! / 1e3, 1)} km, Mach {f(out.chuteOpen[2]!, 2)})</span></div>}
            <div className="card"><b>{f(out.impactSpeed, 1)} m/s</b><span>no solo, {f(out.downrange / 1e3)} km depois, em {f(out.duration / 60, 1)} min</span></div>
          </div>
          {out.warnings.map((w) => <p key={w} className="pcheck bad">{w}</p>)}
          <div className="charts">
            <Chart title="Altitude" unit="km" cursorSync="entry" series={[{ label: "altitude", color: "#5eb1ff", t, y: col(1, 1e-3) }]} />
            <Chart title="Desaceleração" unit="g" cursorSync="entry" series={[{ label: "g", color: "#ff8a3d", t, y: col(4) }]} />
            <Chart title="Fluxo de calor no nariz" unit="MW/m²" cursorSync="entry" series={[{ label: "q", color: "#ff6b6b", t, y: col(5, 1e-6) }]} />
            <Chart title="Velocidade" unit="km/s" cursorSync="entry" series={[{ label: "v", color: "#7ee787", t, y: col(2, 1e-3) }]} />
          </div>
        </>
      )}
    </section>
  );
}

interface LandOut { rows: number[][]; ignitionTime: number; ignitionAlt: number; touchdownSpeed: number; propellantUsed: number; dv: number; success: boolean; message: string }

const LAND_PRESETS: Record<string, Record<string, number | string>> = {
  "Módulo lunar descendo da órbita de 15 km": { body: "lua", altitude: 15000, vx: 1690, vz: 0, mass: 15000, propellant: 8200, thrust: 45000, isp: 311, cdA: 0 },
  "Pouso vertical em Marte depois do paraquedas": { body: "marte", altitude: 2000, vx: 0, vz: -90, mass: 1000, propellant: 250, thrust: 12000, isp: 220, cdA: 4 },
  "Primeiro estágio pousando na Terra": { body: "terra", altitude: 4000, vx: 0, vz: -250, mass: 30000, propellant: 6000, thrust: 845000, isp: 282, cdA: 10 },
};

function Landing({ core, bodies }: { core: CoreClient; bodies: BodyInfo[] }) {
  const [inp, setInp] = useState<Record<string, number | string>>(LAND_PRESETS["Módulo lunar descendo da órbita de 15 km"]!);
  const { out, err } = useTool<LandOut>(core, "landing", inp);
  const n = (k: string) => inp[k] as number;
  const set = (k: string) => (v: number) => setInp({ ...inp, [k]: v });
  const t = out?.rows.map((r) => r[0]!) ?? [];
  const col = (i: number, k = 1) => out?.rows.map((r) => r[i]! * k) ?? [];
  return (
    <section className="panel">
      <div className="orbital-form">
        <label className="field"><span>Exemplo</span><select value="" onChange={(e) => e.target.value && setInp(LAND_PRESETS[e.target.value]!)}><option value="">escolher…</option>{Object.keys(LAND_PRESETS).map((k) => <option key={k}>{k}</option>)}</select></label>
        <BodySelect label="Corpo" value={inp.body as string} bodies={bodies} filter={(b) => b.body.id !== "sol"} onChange={(body) => setInp({ ...inp, body })} />
        <N label="Altitude (m)" value={n("altitude")} onChange={set("altitude")} />
        <N label="Velocidade horizontal (m/s)" value={n("vx")} onChange={set("vx")} />
        <N label="Velocidade vertical (m/s)" value={n("vz")} onChange={set("vz")} />
        <N label="Massa (kg)" value={n("mass")} onChange={set("mass")} />
        <N label="Propelente (kg)" value={n("propellant")} onChange={set("propellant")} />
        <N label="Empuxo máximo (N)" value={n("thrust")} onChange={set("thrust")} />
        <N label="Isp (s)" value={n("isp")} onChange={set("isp")} />
        <N label="Cd·A (m²)" value={n("cdA")} onChange={set("cdA")} />
      </div>
      {err && <p className="bad">{err}</p>}
      {out && (
        <>
          <p className={`verdict ${out.success ? "ok" : "bad"}`}>{out.message}</p>
          <div className="cards">
            <div className="card"><b>{f(out.touchdownSpeed, 1)} m/s</b><span>ao tocar o solo</span></div>
            <div className="card"><b>{f(out.propellantUsed)} kg</b><span>propelente gasto</span></div>
            <div className="card"><b>{f(out.dv)} m/s</b><span>Δv gasto</span></div>
            <div className="card"><b>{f(out.ignitionAlt)} m</b><span>ignição aos {f(out.ignitionTime, 1)} s</span></div>
          </div>
          <div className="charts">
            <Chart title="Altitude" unit="m" cursorSync="land" series={[{ label: "altitude", color: "#5eb1ff", t, y: col(2) }]} />
            <Chart title="Velocidades" unit="m/s" cursorSync="land" series={[{ label: "horizontal", color: "#ff8a3d", t, y: col(3) }, { label: "vertical", color: "#7ee787", t, y: col(4) }]} />
            <Chart title="Aceleração do motor" unit="g" cursorSync="land" series={[{ label: "empuxo/massa", color: "#ff6b6b", t, y: col(6) }]} />
          </div>
        </>
      )}
    </section>
  );
}
