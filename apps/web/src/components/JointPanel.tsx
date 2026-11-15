import { useState } from "react";
import type { Design, Joint, JointKind, JointType, Material } from "@astra/core-js";
import { newJoint } from "../state";
import { jointFields, jointTypeLabel } from "../units";
import { CheckField, NumberField, SelectField } from "./Field";

interface Props {
  design: Design;
  materials: Material[];
  errorJoints: Set<string>;
  selectedPart: string | null;
  onChange: (d: Design) => void;
}

const area = (k: JointKind) => (k.type === "adhesive" && k.area != null ? k.area * 1e6 : null);

export function JointPanel({ design, materials, errorJoints, selectedPart, onChange }: Props) {
  const [open, setOpen] = useState<string | null>(null);
  const [a, setA] = useState("");
  const [b, setB] = useState("");
  const [type, setType] = useState<JointType>("adhesive");
  const name = (id: string) => design.parts.find((p) => p.id === id)?.name ?? id;
  const update = (id: string, fn: (j: Joint) => Joint) => onChange({ ...design, joints: design.joints.map((j) => (j.id === id ? fn(j) : j)) });
  const setKind = (id: string, key: string, v: unknown) => update(id, (j) => ({ ...j, kind: { ...j.kind, [key]: v } as JointKind }));
  const shown = selectedPart ? design.joints.filter((j) => j.a === selectedPart || j.b === selectedPart) : design.joints;
  const adhesives = materials.filter((m) => m.category === "adesivo");
  const metals = materials.filter((m) => m.category === "metal" || m.category === "polimero");

  return (
    <section className="panel joints">
      <h2>Juntas {selectedPart && <small>da peça selecionada</small>}</h2>
      <ul className="joint-list">
        {shown.map((j) => (
          <li key={j.id} className={errorJoints.has(j.id) ? "bad" : ""}>
            <button onClick={() => setOpen(open === j.id ? null : j.id)}>
              <span>{name(j.a)} ↔ {name(j.b)}</span>
              <small>{jointTypeLabel[j.kind.type]}</small>
            </button>
            {open === j.id && (
              <div className="editor">
                <SelectField label="Tipo" value={j.kind.type} options={(Object.keys(jointTypeLabel) as JointType[]).map((t) => ({ value: t, label: jointTypeLabel[t] }))} onChange={(t) => update(j.id, (x) => ({ ...newJoint(design, x.a, x.b, t), id: x.id }))} />
                {j.kind.type === "adhesive" && (
                  <SelectField label="Adesivo" value={j.kind.adhesive} options={adhesives.map((m) => ({ value: m.id, label: m.name }))} onChange={(v) => setKind(j.id, "adhesive", v)} />
                )}
                {(j.kind.type === "screws" || j.kind.type === "shearPins") && (
                  <SelectField label="Material" value={j.kind.material} options={metals.map((m) => ({ value: m.id, label: m.name }))} onChange={(v) => setKind(j.id, "material", v)} />
                )}
                {jointFields[j.kind.type].map((fs) =>
                  fs.key === "area" ? (
                    <NumberField key={fs.key} label="Área colada (mm², opcional)" unit="count" optional help={fs.help} value={area(j.kind)} onChange={(v) => setKind(j.id, "area", v === null ? null : v / 1e6)} />
                  ) : (
                    <NumberField key={fs.key} label={fs.label} unit={fs.unit} help={fs.help} optional={fs.key === "fillet"} value={(j.kind as unknown as Record<string, number | null>)[fs.key] ?? null} onChange={(v) => setKind(j.id, fs.key, fs.unit === "count" && v !== null ? Math.max(1, Math.round(v)) : v)} />
                  ),
                )}
                {j.kind.type === "adhesive" && (
                  <CheckField label="Aleta atravessa o tubo" value={!!j.kind.throughWall} onChange={(v) => setKind(j.id, "throughWall", v)} />
                )}
                <button className="danger" onClick={() => onChange({ ...design, joints: design.joints.filter((x) => x.id !== j.id) })}>Remover junta</button>
              </div>
            )}
          </li>
        ))}
      </ul>
      <div className="add-joint">
        <SelectField label="Ligar" value={a} options={[{ value: "", label: "peça…" }, ...design.parts.map((p) => ({ value: p.id, label: p.name }))]} onChange={setA} />
        <SelectField label="a" value={b} options={[{ value: "", label: "peça…" }, ...design.parts.filter((p) => p.id !== a).map((p) => ({ value: p.id, label: p.name }))]} onChange={setB} />
        <SelectField label="com" value={type} options={(Object.keys(jointTypeLabel) as JointType[]).map((t) => ({ value: t, label: jointTypeLabel[t] }))} onChange={setType} />
        <button disabled={!a || !b} onClick={() => { onChange({ ...design, joints: [...design.joints, newJoint(design, a, b, type)] }); setA(""); setB(""); }}>
          Criar junta
        </button>
      </div>
    </section>
  );
}
