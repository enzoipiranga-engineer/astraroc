import { useEffect, useState } from "react";
import type { CoreClient, Design, Launcher, PluginCheck, PluginSpec, Push } from "@astra/core-js";
import type { Unit } from "../units";
import { NumberField, SelectField, TextField } from "./Field";

type Kind = "cart" | "gas" | "railgun" | "spring" | "hand" | "spin" | "air" | "balloon" | "ship" | "submarine";

const guide = (label: string, push: Push, extra: Partial<Extract<Launcher, { type: "guided" }>> = {}): Launcher => ({
  type: "guided",
  label,
  length: 30,
  elevationDeg: 80,
  azimuthDeg: 0,
  carriageMass: 20,
  friction: 0.003,
  carriageCdA: 0.05,
  push,
  releaseSpeed: null,
  igniteAt: null,
  ignitionDelay: 0,
  brake: 30,
  height: 0,
  ...extra,
});

/** Blocos nativos com valores iniciais plausíveis. */
const BLOCKS: Record<Kind, { label: string; make: () => Launcher }> = {
  cart: { label: "Carrinho em rampa (motor linear)", make: () => guide("Carrinho em rampa", { type: "power", power: 150_000, maxForce: 15_000 }) },
  gas: { label: "Canhão de ar comprimido", make: () => guide("Canhão de ar", { type: "gasGun", p0: 8e5, volume: 0.02, bore: 0.06, gamma: 1.4 }, { length: 3, carriageMass: 0.05, friction: 0.02, carriageCdA: 0 }) },
  railgun: { label: "Railgun / coilgun", make: () => guide("Railgun", { type: "railgun", current: 2e5, inductanceGradient: 0.5e-6 }, { length: 10, carriageMass: 1, carriageCdA: 0 }) },
  spring: { label: "Mola, estilingue ou catapulta", make: () => guide("Mola", { type: "spring", stiffness: 3000, stroke: 0.6 }, { length: 0.6, carriageMass: 0.2, carriageCdA: 0, elevationDeg: 70 }) },
  hand: { label: "Mão (arremesso)", make: () => guide("Arremesso", { type: "constant", force: 150 }, { length: 0.8, carriageMass: 0, carriageCdA: 0, elevationDeg: 60, height: 1.7, friction: 0 }) },
  spin: { label: "Plataforma giratória", make: () => ({ type: "spin", radius: 10, armMass: 500, torque: 20_000, targetRpm: 200, chamberPressure: 0, armCdWidth: 0.3, releaseElevationDeg: 70, azimuthDeg: 0, hubHeight: 12, ignitionDelay: 0.5 }) },
  air: { label: "Lançamento aéreo (avião)", make: () => ({ type: "air", altitude: 3000, speed: 120, flightPathDeg: 15, headingDeg: 0, dropTime: 1, aircraftMass: 2000 }) },
  balloon: { label: "Balão estratosférico (rockoon)", make: () => ({ type: "balloon", envelopeMass: 3, gasMass: 2, gasMolarMass: 4.0026, gondolaMass: 2, burstDiameter: 12, cd: 0.3, releaseAltitude: 15_000, railElevationDeg: 85, railAzimuthDeg: 0 }) },
  ship: { label: "Navio", make: () => ({ type: "ship", speed: 8, headingDeg: 0, pitchAmplitudeDeg: 3, rollAmplitudeDeg: 5, wavePeriod: 8, launchPhase: 1, deckHeight: 6, railLength: 3, railElevationDeg: 85 }) },
  submarine: { label: "Submarino (lançamento a frio)", make: () => ({ type: "submarine", depth: 15, ejectionSpeed: 25, waterCd: 0.3, ignitionDelay: 0.3 }) },
};

/** Campos editáveis por tipo: [chave, rótulo, unidade]. */
const FIELDS: Record<string, [string, string, Unit][]> = {
  guided: [["length", "Comprimento da guia", "m"], ["elevationDeg", "Elevação", "deg"], ["azimuthDeg", "Azimute", "deg"], ["carriageMass", "Massa do carrinho/pistão", "kg"], ["friction", "Atrito", "ratio"], ["carriageCdA", "Cd·A do carrinho (m²)", "ratio"], ["height", "Altura da guia", "m"], ["ignitionDelay", "Ignição após a liberação", "s"], ["brake", "Frenagem do carrinho (m/s²)", "ratio"]],
  spin: [["radius", "Raio do braço", "m"], ["armMass", "Massa do braço", "kg"], ["torque", "Torque do motor (N·m)", "ratio"], ["targetRpm", "Rotação (rpm)", "ratio"], ["chamberPressure", "Pressão na câmara (Pa, 0 = vácuo)", "ratio"], ["releaseElevationDeg", "Elevação na liberação", "deg"], ["azimuthDeg", "Azimute", "deg"], ["hubHeight", "Altura do eixo", "m"], ["ignitionDelay", "Ignição após a liberação", "s"]],
  air: [["altitude", "Altura de liberação", "m"], ["speed", "Velocidade do avião (m/s)", "ratio"], ["flightPathDeg", "Ângulo de subida", "deg"], ["headingDeg", "Proa", "deg"], ["dropTime", "Queda livre até a ignição", "s"], ["aircraftMass", "Massa do avião", "kg"]],
  balloon: [["releaseAltitude", "Altura de liberação", "m"], ["gasMass", "Massa de hélio", "kg"], ["envelopeMass", "Massa do envelope", "kg"], ["gondolaMass", "Massa da gôndola", "kg"], ["burstDiameter", "Diâmetro de estouro", "m"], ["railElevationDeg", "Elevação do trilho", "deg"]],
  ship: [["speed", "Velocidade do navio (m/s)", "ratio"], ["headingDeg", "Rumo", "deg"], ["pitchAmplitudeDeg", "Arfagem", "deg"], ["rollAmplitudeDeg", "Balanço", "deg"], ["wavePeriod", "Período das ondas", "s"], ["launchPhase", "Instante no ciclo", "s"], ["railLength", "Trilho no convés", "m"], ["railElevationDeg", "Elevação do trilho", "deg"]],
  submarine: [["depth", "Profundidade", "m"], ["ejectionSpeed", "Velocidade de ejeção (m/s)", "ratio"], ["waterCd", "Cd na água", "ratio"], ["ignitionDelay", "Ignição após a superfície", "s"]],
};

const PUSH_FIELDS: Record<string, [string, string][]> = {
  constant: [["force", "Força (N)"]],
  power: [["power", "Potência (W)"], ["maxForce", "Força máxima (N)"]],
  gasGun: [["p0", "Pressão inicial absoluta (Pa)"], ["volume", "Volume do reservatório (m³)"], ["bore", "Diâmetro do cano (m)"]],
  railgun: [["current", "Corrente (A)"], ["inductanceGradient", "Gradiente de indutância (H/m)"]],
  spring: [["stiffness", "Rigidez (N/m)"], ["stroke", "Curso (m)"]],
};

const label = (l: Launcher) =>
  l.type === "guided" ? l.label : l.type === "plugin" ? l.spec.name : { spin: "Plataforma giratória", air: "Avião", balloon: "Balão", ship: "Navio", submarine: "Submarino" }[l.type];

const PLUGIN_KEY = "astra.plugins.v1";
const loadUserPlugins = (): PluginSpec[] => {
  try {
    return JSON.parse(localStorage.getItem(PLUGIN_KEY) ?? "[]") as PluginSpec[];
  } catch {
    return [];
  }
};

/**
 * Sistema de lançamento: cadeia de blocos (arrastáveis), cada um com seus parâmetros, e a
 * biblioteca de plugins da comunidade.
 */
export function LauncherPanel({ design, core, onChange }: { design: Design; core: CoreClient; onChange: (d: Design) => void }) {
  const chain = design.launcher ?? [];
  const setChain = (c: Launcher[]) => onChange({ ...design, launcher: c });
  const [open, setOpen] = useState<number | null>(null);
  const [drag, setDrag] = useState<number | null>(null);
  const [builtin, setBuiltin] = useState<PluginSpec[]>([]);
  const [user, setUser] = useState<PluginSpec[]>(loadUserPlugins);
  const [filter, setFilter] = useState("");
  const [checks, setChecks] = useState<Record<string, PluginCheck>>({});
  useEffect(() => {
    core.plugins().then(setBuiltin, () => undefined);
  }, [core]);
  const saveUser = (ps: PluginSpec[]) => {
    setUser(ps);
    try {
      localStorage.setItem(PLUGIN_KEY, JSON.stringify(ps));
    } catch {
      /* sem armazenamento */
    }
  };
  // Plugin compartilhado por link (#plugin=base64).
  useEffect(() => {
    const m = /#plugin=([^&]+)/.exec(window.location.hash);
    if (!m) return;
    try {
      const spec = JSON.parse(decodeURIComponent(escape(atob(m[1]!)))) as PluginSpec;
      if (!user.some((p) => p.id === spec.id)) saveUser([...user, spec]);
    } catch {
      /* link inválido */
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const update = (i: number, patch: Record<string, unknown>) => setChain(chain.map((l, k) => (k === i ? ({ ...l, ...patch } as Launcher) : l)));
  const move = (from: number, to: number) => {
    const c = [...chain];
    const [x] = c.splice(from, 1);
    c.splice(to, 0, x!);
    setChain(c);
  };
  const check = async (p: PluginSpec) => setChecks({ ...checks, [p.id]: await core.checkPlugin(p) });
  const importPlugin = async (f: File | undefined) => {
    if (!f) return;
    try {
      const spec = JSON.parse(await f.text()) as PluginSpec;
      const c = await core.checkPlugin(spec);
      setChecks({ ...checks, [spec.id]: c });
      saveUser([...user.filter((p) => p.id !== spec.id), spec]);
    } catch {
      alert("Arquivo de plugin inválido.");
    }
  };
  const exportPlugin = (p: PluginSpec) => {
    const url = URL.createObjectURL(new Blob([JSON.stringify(p, null, 2)], { type: "application/json" }));
    Object.assign(document.createElement("a"), { href: url, download: `${p.id.replace(/\W+/g, "_")}.plugin.json` }).click();
    URL.revokeObjectURL(url);
  };
  const shareLink = (p: PluginSpec) => {
    const link = `${window.location.origin}${window.location.pathname}#plugin=${btoa(unescape(encodeURIComponent(JSON.stringify(p))))}`;
    void navigator.clipboard?.writeText(link);
    alert("Link do plugin copiado.");
  };
  const library = [...builtin, ...user].filter((p) => `${p.name} ${p.description} ${p.author}`.toLowerCase().includes(filter.toLowerCase()));

  return (
    <section className="panel launcher">
      <h2>Sistema de lançamento</h2>
      {chain.length === 0 ? (
        <p className="hint">Trilho comum (configurado em Motor e lançador). Adicione blocos para outro método.</p>
      ) : (
        <ol className="chain">
          {chain.map((l, i) => (
            <li
              key={i}
              draggable
              onDragStart={() => setDrag(i)}
              onDragOver={(e) => e.preventDefault()}
              onDrop={() => {
                if (drag !== null && drag !== i) move(drag, i);
                setDrag(null);
              }}
              className={drag === i ? "dragging" : ""}
            >
              <div className="chain-head">
                <span className="grip" aria-hidden>⠿</span>
                <button className="link" onClick={() => setOpen(open === i ? null : i)}>{i + 1}. {label(l)}</button>
                <button aria-label="Subir" disabled={i === 0} onClick={() => move(i, i - 1)}>↑</button>
                <button aria-label="Descer" disabled={i === chain.length - 1} onClick={() => move(i, i + 1)}>↓</button>
                <button aria-label="Remover" onClick={() => setChain(chain.filter((_, k) => k !== i))}>×</button>
              </div>
              {open === i && <BlockEditor l={l} onPatch={(p) => update(i, p)} />}
            </li>
          ))}
        </ol>
      )}
      <SelectField
        label="Adicionar bloco"
        value={"" as Kind | ""}
        options={[{ value: "", label: "escolha…" }, ...(Object.keys(BLOCKS) as Kind[]).map((k) => ({ value: k, label: BLOCKS[k].label }))]}
        onChange={(k) => {
          if (k) {
            setChain([...chain, BLOCKS[k as Kind].make()]);
            setOpen(chain.length);
          }
        }}
      />
      {chain.length > 1 && <p className="hint">Em cadeia, cada bloco parte do estado em que o anterior soltou o foguete (as velocidades se somam).</p>}

      <details className="plugins">
        <summary>Biblioteca de plugins ({builtin.length + user.length})</summary>
        <div className="search">
          <input value={filter} onChange={(e) => setFilter(e.target.value)} placeholder="Buscar plugin…" aria-label="Buscar plugin" />
          <label className="button">Importar<input type="file" accept=".json" hidden onChange={(e) => void importPlugin(e.target.files?.[0])} /></label>
        </div>
        <ul className="plugin-list">
          {library.map((p) => {
            const c = checks[p.id];
            const mine = user.some((u) => u.id === p.id);
            return (
              <li key={p.id}>
                <b>{p.name}</b> <small>v{p.version} · {p.author}</small>
                <p className="hint">{p.description}<br /><em>Validade: {p.validity}</em></p>
                <div className="row-buttons">
                  <button onClick={() => setChain([...chain, { type: "plugin", spec: p, values: {} }])}>Usar</button>
                  <button onClick={() => void check(p)}>Verificar</button>
                  <button onClick={() => saveUser([...user, { ...p, id: `${p.id}-copia-${Date.now() % 10000}`, name: `${p.name} (cópia)`, author: "eu" }])}>Duplicar</button>
                  <button onClick={() => exportPlugin(p)}>Exportar</button>
                  <button onClick={() => shareLink(p)}>Link</button>
                  {mine && <button className="danger" onClick={() => saveUser(user.filter((u) => u.id !== p.id))}>Remover</button>}
                </div>
                {c && <ul className={`pcheck ${c.ok ? "ok" : "bad"}`}>{c.messages.map((m, k) => <li key={k}>{m}</li>)}</ul>}
              </li>
            );
          })}
        </ul>
        <p className="note">
          Crie o seu: copie um plugin, mude a força (expressão em x, v, t, m e nos parâmetros), declare unidades e faixa de validade e
          clique em Verificar. Expressões não têm acesso a rede nem a arquivos e têm tamanho limitado.
        </p>
      </details>
      <p className="note">
        O ASTRA é ferramenta de estudo e projeto: não substitui análise de segurança, autorização de espaço aéreo nem as normas de lançamento locais.
      </p>
    </section>
  );
}

function BlockEditor({ l, onPatch }: { l: Launcher; onPatch: (p: Record<string, unknown>) => void }) {
  if (l.type === "plugin") {
    return (
      <div className="editor">
        <p className="hint">{l.spec.description}</p>
        {l.spec.params.map((p) => (
          <NumberField key={p.name} label={`${p.name} (${p.unit})`} unit="ratio" help={`${p.help} — faixa ${p.min} a ${p.max}`} value={l.values[p.name] ?? p.value} onChange={(v) => v !== null && onPatch({ values: { ...l.values, [p.name]: v } })} />
        ))}
      </div>
    );
  }
  const rec = l as unknown as Record<string, number>;
  return (
    <div className="editor">
      {l.type === "guided" && <TextField label="Nome" value={l.label} onChange={(v) => onPatch({ label: v })} />}
      {(FIELDS[l.type] ?? []).map(([k, lab, u]) => (
        <NumberField key={k} label={lab} unit={u} value={rec[k] ?? 0} onChange={(v) => v !== null && onPatch({ [k]: v })} />
      ))}
      {l.type === "guided" && (
        <>
          {(PUSH_FIELDS[l.push.type] ?? []).map(([k, lab]) => (
            <NumberField key={k} label={lab} unit="ratio" value={(l.push as unknown as Record<string, number>)[k] ?? 0} onChange={(v) => v !== null && onPatch({ push: { ...l.push, [k]: v } })} />
          ))}
          <label className="field check">
            <input type="checkbox" checked={l.igniteAt != null} onChange={(e) => onPatch({ igniteAt: e.target.checked ? 0 : null })} />
            <span>Acender o motor ainda na guia</span>
          </label>
        </>
      )}
    </div>
  );
}
