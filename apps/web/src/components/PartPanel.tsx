import type { Design, Material, Part, PartKind, PartType } from "@astra/core-js";
import { newPart } from "../state";
import { partFields, partTypeLabel } from "../units";
import { CheckField, NumberField, SelectField, TextField } from "./Field";

interface Props {
  design: Design;
  materials: Material[];
  selected: string | null;
  errorParts: Set<string>;
  onSelect: (id: string | null) => void;
  onChange: (d: Design) => void;
}

const reliabilityLabel = { medido: "medido", catalogo: "catálogo", estimado: "estimado" } as const;

export function PartPanel({ design, materials, selected, errorParts, onSelect, onChange }: Props) {
  const part = design.parts.find((p) => p.id === selected) ?? null;
  const update = (id: string, fn: (p: Part) => Part) =>
    onChange({ ...design, parts: design.parts.map((p) => (p.id === id ? fn(p) : p)) });
  const setKind = (id: string, key: string, v: unknown) =>
    update(id, (p) => ({ ...p, kind: { ...p.kind, [key]: v } as PartKind }));
  const add = (type: PartType) => {
    const p = newPart(design, type);
    onChange({ ...design, parts: [...design.parts, p] });
    onSelect(p.id);
  };
  const remove = (id: string) => {
    onChange({ ...design, parts: design.parts.filter((p) => p.id !== id), joints: design.joints.filter((j) => j.a !== id && j.b !== id) });
    onSelect(null);
  };
  const mat = part ? materials.find((m) => m.id === part.material) : undefined;
  const sorted = [...design.parts].sort((a, b) => a.position - b.position);

  return (
    <section className="panel parts">
      <h2>Peças</h2>
      <ul className="part-list">
        {sorted.map((p) => (
          <li key={p.id}>
            <button className={[p.id === selected ? "sel" : "", errorParts.has(p.id) ? "bad" : ""].join(" ")} onClick={() => onSelect(p.id)}>
              <span>{p.name}</span>
              <small>{partTypeLabel[p.kind.type]}</small>
            </button>
          </li>
        ))}
      </ul>
      <label className="field add">
        <span>Adicionar peça</span>
        <select value="" onChange={(e) => e.target.value && add(e.target.value as PartType)}>
          <option value="">escolha o tipo…</option>
          {(Object.keys(partTypeLabel) as PartType[]).map((t) => (
            <option key={t} value={t}>{partTypeLabel[t]}</option>
          ))}
        </select>
      </label>

      {part && (
        <div className="editor" key={part.id}>
          <h3>{part.name}</h3>
          <TextField label="Nome" value={part.name} onChange={(v) => update(part.id, (p) => ({ ...p, name: v }))} />
          <SelectField
            label="Material"
            value={part.material}
            options={materials.map((m) => ({ value: m.id, label: m.name, group: m.category }))}
            onChange={(v) => update(part.id, (p) => ({ ...p, material: v }))}
          />
          {mat && (
            <p className="hint">
              {mat.densityKgM3} kg/m³ · E {mat.eGPa} GPa · escoamento {mat.yieldMPa} MPa · dado{" "}
              <b className={`rel ${mat.reliability}`}>{reliabilityLabel[mat.reliability]}</b>
              <br />
              <small>{mat.source}</small>
              {mat.notes && (
                <>
                  <br />
                  <small>{mat.notes}</small>
                </>
              )}
            </p>
          )}
          <NumberField label={part.kind.type === "finSet" ? "Posição do bordo de ataque" : "Posição (frente)"} unit="mm" value={part.position} help="Distância a partir da ponta do nariz." onChange={(v) => v !== null && update(part.id, (p) => ({ ...p, position: v }))} />
          {partFields[part.kind.type].map((fs) => (
            <NumberField key={fs.key} label={fs.label} unit={fs.unit} help={fs.help} value={(part.kind as unknown as Record<string, number>)[fs.key]} onChange={(v) => v !== null && setKind(part.id, fs.key, fs.unit === "count" ? Math.max(0, Math.round(v)) : v)} />
          ))}
          {part.kind.type === "noseCone" && (
            <SelectField
              label="Perfil"
              value={part.kind.shape}
              options={[
                { value: "tangentOgive", label: "Ogiva tangente" },
                { value: "vonKarman", label: "Von Kármán" },
                { value: "lvHaack", label: "Haack LV" },
                { value: "parabolic", label: "Parabólica" },
                { value: "elliptical", label: "Elíptica" },
                { value: "conical", label: "Cônica" },
              ]}
              onChange={(v) => setKind(part.id, "shape", v)}
            />
          )}
          {part.kind.type === "finSet" && (
            <SelectField label="Seção" value={part.kind.profile} options={[{ value: "rounded", label: "Bordos arredondados" }, { value: "square", label: "Bordos retos" }, { value: "airfoil", label: "Perfil aerodinâmico" }]} onChange={(v) => setKind(part.id, "profile", v)} />
          )}
          {part.kind.type === "mass" && (
            <CheckField label="Mede pressão (altímetro)" value={part.kind.needsStaticPort} help="Exige furo de pressão estática no tubo." onChange={(v) => setKind(part.id, "needsStaticPort", v)} />
          )}
          {part.kind.type === "parachute" && (
            <ChuteFields design={design} part={part} kind={part.kind} setKind={(k, v) => setKind(part.id, k, v)} />
          )}
          <NumberField label="Massa medida" unit="g" optional value={part.massOverride ?? null} help="Se informada, substitui a massa calculada pelo material." onChange={(v) => update(part.id, (p) => ({ ...p, massOverride: v }))} />
          <NumberField label="Custo" unit="brl" optional value={part.cost ?? null} onChange={(v) => update(part.id, (p) => ({ ...p, cost: v }))} />
          <button className="danger" onClick={() => remove(part.id)}>Remover peça</button>
        </div>
      )}
    </section>
  );
}

function ChuteFields({ design, kind, setKind }: { design: Design; part: Part; kind: Extract<PartKind, { type: "parachute" }>; setKind: (k: string, v: unknown) => void }) {
  const d = kind.deploy;
  return (
    <>
      <SelectField
        label="Quando ejetar"
        value={d.type}
        options={[{ value: "apogee", label: "No apogeu + atraso" }, { value: "altitude", label: "Descendo por uma altura" }, { value: "time", label: "Tempo após a ignição" }]}
        onChange={(t) => setKind("deploy", t === "apogee" ? { type: t, delay: 1 } : t === "altitude" ? { type: t, agl: 150 } : { type: t, t: 8 })}
      />
      {d.type === "apogee" && <NumberField label="Atraso" unit="s" value={d.delay} onChange={(v) => v !== null && setKind("deploy", { ...d, delay: v })} />}
      {d.type === "altitude" && <NumberField label="Altura acima do solo" unit="m" value={d.agl} onChange={(v) => v !== null && setKind("deploy", { ...d, agl: v })} />}
      {d.type === "time" && <NumberField label="Tempo" unit="s" value={d.t} onChange={(v) => v !== null && setKind("deploy", { ...d, t: v })} />}
      <SelectField
        label="Junta que abre a baia"
        value={kind.separationJoint ?? ""}
        options={[{ value: "", label: "nenhuma" }, ...design.joints.filter((j) => j.kind.type !== "cord").map((j) => ({ value: j.id, label: j.id }))]}
        onChange={(v) => setKind("separationJoint", v || null)}
      />
    </>
  );
}
