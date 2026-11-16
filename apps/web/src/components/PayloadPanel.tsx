import { useState } from "react";
import type { Design, Metric, Part, Payload, PayloadPreset, Requirements, Rule, RuleSet } from "@astra/core-js";
import { CheckField, NumberField, SelectField, TextField } from "./Field";

interface Props {
  design: Design;
  presets: PayloadPreset[];
  rulePresets: [string, RuleSet][];
  onChange: (d: Design) => void;
}

const REQS: { key: keyof Requirements; label: string; unit: string }[] = [
  { key: "maxAxialG", label: "Aceleração axial máx.", unit: "g" },
  { key: "maxLateralG", label: "Aceleração lateral máx.", unit: "g" },
  { key: "maxShockG", label: "Choque máx.", unit: "g" },
  { key: "minTempC", label: "Temperatura mín.", unit: "°C" },
  { key: "maxTempC", label: "Temperatura máx.", unit: "°C" },
  { key: "maxDpDt", label: "Variação de pressão máx.", unit: "Pa/s" },
  { key: "maxBayDp", label: "ΔP na baia máx.", unit: "Pa" },
  { key: "maxLandingSpeed", label: "Velocidade de pouso máx.", unit: "m/s" },
  { key: "maxSplDb", label: "Ruído máx.", unit: "dB" },
  { key: "qualifiedGrms", label: "Qualificada a", unit: "Grms" },
];

export const METRICS: Record<Metric, { label: string; unit: string }> = {
  apogeeAgl: { label: "Apogeu acima do solo", unit: "m" },
  apogeeErrorPct: { label: "Erro do apogeu", unit: "%" },
  railExitSpeed: { label: "Velocidade na saída do trilho", unit: "m/s" },
  staticMargin: { label: "Margem estática na decolagem", unit: "cal" },
  thrustToWeight: { label: "Empuxo/peso", unit: "" },
  maxMach: { label: "Mach máximo", unit: "" },
  liftoffMass: { label: "Massa na decolagem", unit: "kg" },
  landingSpeed: { label: "Velocidade de pouso", unit: "m/s" },
  landingEnergy: { label: "Energia no pouso", unit: "J" },
  drift: { label: "Distância de pouso", unit: "m" },
  payloadMass: { label: "Massa do payload", unit: "kg" },
  payloadCubeForm: { label: "Payload em Cube Units", unit: "" },
  structureIntact: { label: "Estrutura íntegra", unit: "" },
  declared: { label: "Declaração da equipe", unit: "" },
};

const mounts = (d: Design) => d.parts.filter((p) => ["bulkhead", "coupler", "bodyTube", "noseCone", "centeringRing"].includes(p.kind.type));

/** Número opcional sem conversão de unidade (o rótulo leva a unidade). */
function Num({ label, unit, value, onChange }: { label: string; unit: string; value: number | null | undefined; onChange: (v: number | null) => void }) {
  return <NumberField label={unit ? `${label} (${unit})` : label} unit="count" optional placeholder="sem limite" value={value ?? null} onChange={onChange} />;
}

export function PayloadPanel({ design, presets, rulePresets, onChange }: Props) {
  const [preset, setPreset] = useState(presets[0]?.id ?? "");
  const [mount, setMount] = useState(() => mounts(design)[0]?.id ?? "");
  const [newMetric, setNewMetric] = useState<Metric>("railExitSpeed");
  const setPayload = (id: string, f: (p: Payload) => Payload) => onChange({ ...design, payloads: design.payloads.map((p) => (p.id === id ? f(p) : p)) });

  const add = () => {
    const pr = presets.find((p) => p.id === preset);
    const at = design.parts.find((p) => p.id === mount);
    if (!pr || !at) return;
    const ids = new Set([...design.parts.map((p) => p.id), ...design.payloads.map((p) => p.id)]);
    let n = 1;
    while (ids.has(`carga${n}`)) n++;
    const id = `carga${n}`;
    const part: Part = { id, name: pr.name, material: "al-6061-t6", position: at.position, massOverride: pr.mass, kind: { type: "mass", mass: pr.mass, length: pr.length, diameter: pr.width, needsStaticPort: false } };
    onChange({
      ...design,
      parts: [...design.parts, part],
      joints: [...design.joints, { id: `j-${id}`, a: id, b: at.id, kind: { type: "screws", count: 4, diameter: 0.004, material: "aco-parafuso-8.8" } }],
      payloads: [...design.payloads, { id, name: pr.name, kind: pr.kind, part: id, functional: false, requirements: { ...pr.requirements } }],
    });
  };
  const remove = (p: Payload) =>
    onChange({
      ...design,
      payloads: design.payloads.filter((x) => x.id !== p.id),
      parts: design.parts.filter((x) => x.id !== p.part),
      joints: design.joints.filter((j) => j.a !== p.part && j.b !== p.part),
    });

  const rs = design.competition ?? null;
  const setRs = (r: RuleSet | null) => onChange({ ...design, competition: r });
  const setRule = (i: number, r: Rule) => rs && setRs({ ...rs, rules: rs.rules.map((x, k) => (k === i ? r : x)) });
  const addRule = () => {
    if (!rs) return;
    const r: Rule = { id: `regra${rs.rules.length + 1}`, text: METRICS[newMetric].label, metric: newMetric, op: "atLeast", min: null, max: null, penalty: 0, blocking: false, declared: false, source: "informada pela equipe" };
    setRs({ ...rs, rules: [...rs.rules, r] });
  };

  return (
    <section className="panel payload">
      <h2>
        Carga útil e regras <small>ambiente de voo e pontuação</small>
      </h2>
      <div className="elec-grid">
        <div>
          <h3>Cargas úteis</h3>
          {design.payloads.length === 0 && <p className="hint">Nenhuma carga útil. Escolha um modelo e onde fixá-lo.</p>}
          {design.payloads.map((p) => (
            <details key={p.id} className="payload-item" open>
              <summary>
                <b>{p.name}</b> <small>({design.parts.find((x) => x.id === p.part)?.name ?? p.part})</small>
              </summary>
              <TextField label="Nome" value={p.name} onChange={(v) => setPayload(p.id, (x) => ({ ...x, name: v }))} />
              <CheckField label="Funcional (experimento ativo)" value={p.functional} onChange={(v) => setPayload(p.id, (x) => ({ ...x, functional: v }))} />
              <div className="row">
                {REQS.map((r) => (
                  <Num key={r.key} label={r.label} unit={r.unit} value={p.requirements[r.key]} onChange={(v) => setPayload(p.id, (x) => ({ ...x, requirements: { ...x.requirements, [r.key]: v } }))} />
                ))}
              </div>
              <p className="hint">Massa, tamanho e posição ficam na peça "{design.parts.find((x) => x.id === p.part)?.name}" (lista de peças).</p>
              <button className="link danger" onClick={() => remove(p)}>remover carga</button>
            </details>
          ))}
          <div className="row">
            <SelectField label="Modelo" value={preset} options={presets.map((p) => ({ value: p.id, label: p.name }))} onChange={setPreset} />
            <SelectField label="Fixada (4 parafusos M4) em" value={mount} options={mounts(design).map((p) => ({ value: p.id, label: p.name }))} onChange={setMount} />
          </div>
          {presets.find((p) => p.id === preset) && <p className="hint">{presets.find((p) => p.id === preset)!.source}</p>}
          <button onClick={add}>Adicionar carga útil</button>
        </div>

        <div className="rules-col">
          <h3>Regras da competição</h3>
          <SelectField
            label="Conjunto de regras"
            value=""
            options={[{ value: "", label: rs ? `${rs.name} (trocar…)` : "nenhum (escolher…)" }, ...rulePresets.map(([id, r]) => ({ value: id, label: r.name })), ...(rs ? [{ value: "__none", label: "sem regras" }] : [])]}
            onChange={(v) => (v === "__none" ? setRs(null) : v && setRs(structuredClone(rulePresets.find(([id]) => id === v)![1])))}
          />
          {rs && (
            <>
              <p className="hint">{rs.document}</p>
              <Num label="Apogeu-alvo" unit="m" value={rs.targetApogee} onChange={(v) => setRs({ ...rs, targetApogee: v })} />
              {rs.pending.map((p) => (
                <p key={p} className="pcheck bad">Pendente: {p}</p>
              ))}
              <table className="rules">
                <tbody>
                  {rs.rules.map((r, i) => (
                    <tr key={r.id}>
                      <td>
                        {r.text}
                        <br />
                        <small>
                          {METRICS[r.metric].label} · {r.source}
                        </small>
                      </td>
                      <td>
                        {r.metric === "declared" ? (
                          <label className="pcheck-inline">
                            <input type="checkbox" checked={r.declared} onChange={(e) => setRule(i, { ...r, declared: e.target.checked })} /> atende
                          </label>
                        ) : (
                          <>
                            <select value={r.op} onChange={(e) => setRule(i, { ...r, op: e.target.value as Rule["op"] })}>
                              <option value="atLeast">≥</option>
                              <option value="atMost">≤</option>
                              <option value="between">entre</option>
                            </select>
                            {r.op !== "atMost" && <input className="tiny" type="number" value={r.min ?? ""} onChange={(e) => setRule(i, { ...r, min: e.target.value === "" ? null : Number(e.target.value) })} />}
                            {r.op !== "atLeast" && <input className="tiny" type="number" value={r.max ?? ""} onChange={(e) => setRule(i, { ...r, max: e.target.value === "" ? null : Number(e.target.value) })} />}
                            <small> {METRICS[r.metric].unit}</small>
                          </>
                        )}
                      </td>
                      <td>
                        −<input className="tiny" type="number" value={r.penalty} onChange={(e) => setRule(i, { ...r, penalty: Number(e.target.value) || 0 })} /> pts
                        <label className="pcheck-inline" title="Violação impede o lançamento">
                          <input type="checkbox" checked={r.blocking} onChange={(e) => setRule(i, { ...r, blocking: e.target.checked })} /> segurança
                        </label>
                      </td>
                      <td>
                        <button className="link danger" onClick={() => setRs({ ...rs, rules: rs.rules.filter((_, k) => k !== i) })}>×</button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <div className="row">
                <SelectField label="Nova regra" value={newMetric} options={(Object.keys(METRICS) as Metric[]).map((m) => ({ value: m, label: METRICS[m].label }))} onChange={setNewMetric} />
                <button onClick={addRule}>Acrescentar</button>
              </div>
            </>
          )}
        </div>
      </div>
    </section>
  );
}
