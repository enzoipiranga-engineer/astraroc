import type { Design, MotorSpec } from "@astra/core-js";
import { NumberField, SelectField } from "./Field";

/** Locais pré-definidos (coordenadas aproximadas). */
export const SITES = [
  { id: "sp", name: "São Paulo (SP)", lat: -23.55, lon: -46.63, alt: 760 },
  { id: "alcantara", name: "Alcântara (MA), aprox.", lat: -2.32, lon: -44.37, alt: 40 },
  { id: "kourou", name: "Kourou, aprox.", lat: 5.24, lon: -52.77, alt: 10 },
  { id: "canaveral", name: "Cabo Canaveral, aprox.", lat: 28.49, lon: -80.58, alt: 3 },
  { id: "spaceport", name: "Spaceport America", lat: 32.99, lon: -106.975, alt: 1400 },
  { id: "blackrock", name: "Black Rock Desert, aprox.", lat: 40.9, lon: -119.06, alt: 1190 },
  { id: "esrange", name: "Esrange, aprox.", lat: 67.89, lon: 21.11, alt: 330 },
] as const;

interface Props {
  design: Design;
  onChange: (d: Design) => void;
}

export function SetupPanel({ design, onChange }: Props) {
  const l = design.launch;
  const setL = (k: keyof Design["launch"], v: number | null) => v !== null && onChange({ ...design, launch: { ...l, [k]: v } });
  const m = design.motor;
  const setM = (patch: Partial<MotorSpec>) => onChange({ ...design, motor: { ...m, ...patch } as MotorSpec });
  const site = SITES.find((s) => Math.abs(s.lat - l.latDeg) < 1e-6 && Math.abs(s.lon - l.lonDeg) < 1e-6)?.id ?? "";

  const onFile = async (f: File | undefined) => {
    if (!f) return;
    const text = await f.text();
    const aft = m.type === "none" ? 1 : m.aftStation;
    onChange({ ...design, motor: { type: "eng", text, aftStation: aft } });
  };
  const header = m.type === "eng" ? m.text.split("\n").find((x) => x.trim() && !x.trim().startsWith(";")) ?? "" : "";

  return (
    <section className="panel setup">
      <h2>Motor e lançamento</h2>
      <SelectField
        label="Motor"
        value={m.type}
        options={[{ value: "eng", label: "Arquivo .eng (sólido)" }, { value: "water", label: "Foguete de água" }, { value: "none", label: "Sem motor" }]}
        onChange={(t) =>
          onChange({
            ...design,
            motor:
              t === "water"
                ? { type: "water", bottleVolume: 2e-3, waterVolume: 0.66e-3, gaugePressure: 4e5, nozzleDiameter: 0.0215, maxSafePressure: 7e5, aftStation: m.type === "none" ? 0.5 : m.aftStation }
                : t === "eng"
                  ? { type: "eng", text: m.type === "eng" ? m.text : "", aftStation: m.type === "none" ? 1 : m.aftStation }
                  : { type: "none" },
          })
        }
      />
      {m.type === "eng" && (
        <>
          <p className="hint mono">{header || "nenhum arquivo"}</p>
          <label className="field">
            <span>Carregar .eng (thrustcurve.org)</span>
            <input type="file" accept=".eng,.txt" onChange={(e) => void onFile(e.target.files?.[0])} />
          </label>
          <NumberField label="Tubeira na estação" unit="mm" value={m.aftStation} onChange={(v) => v !== null && setM({ aftStation: v })} />
        </>
      )}
      {m.type === "water" && (
        <>
          <NumberField label="Volume da garrafa" unit="L" value={m.bottleVolume} onChange={(v) => v !== null && setM({ bottleVolume: v })} />
          <NumberField label="Água" unit="L" value={m.waterVolume} help="Cerca de 1/3 da garrafa costuma dar o maior apogeu." onChange={(v) => v !== null && setM({ waterVolume: v })} />
          <NumberField label="Pressão manométrica" unit="bar" value={m.gaugePressure} onChange={(v) => v !== null && setM({ gaugePressure: v })} />
          <NumberField label="Pressão máxima segura" unit="bar" value={m.maxSafePressure} help="Garrafas PET costumam romper entre 10 e 12 bar. Nunca pressurize perto de pessoas." onChange={(v) => v !== null && setM({ maxSafePressure: v })} />
          <NumberField label="Bocal" unit="mm" value={m.nozzleDiameter} onChange={(v) => v !== null && setM({ nozzleDiameter: v })} />
          <NumberField label="Bocal na estação" unit="mm" value={m.aftStation} onChange={(v) => v !== null && setM({ aftStation: v })} />
        </>
      )}
      <SelectField
        label="Local"
        value={site}
        options={[{ value: "", label: "personalizado" }, ...SITES.map((s) => ({ value: s.id, label: s.name }))]}
        onChange={(id) => {
          const s = SITES.find((x) => x.id === id);
          if (s) onChange({ ...design, launch: { ...l, latDeg: s.lat, lonDeg: s.lon, altitude: s.alt } });
        }}
      />
      <div className="row">
        <NumberField label="Latitude" unit="deg" value={l.latDeg} onChange={(v) => setL("latDeg", v)} />
        <NumberField label="Longitude" unit="deg" value={l.lonDeg} onChange={(v) => setL("lonDeg", v)} />
      </div>
      <NumberField label="Altitude do solo" unit="m" value={l.altitude} onChange={(v) => setL("altitude", v)} />
      <div className="row">
        <NumberField label="Trilho" unit="m" value={l.railLength} onChange={(v) => setL("railLength", v)} />
        <NumberField label="Elevação" unit="deg" value={l.railElevationDeg} onChange={(v) => setL("railElevationDeg", v)} />
      </div>
      <div className="row">
        <NumberField label="Azimute" unit="deg" value={l.railAzimuthDeg} help="A partir do norte, sentido horário." onChange={(v) => setL("railAzimuthDeg", v)} />
        <NumberField label="Desvio de temperatura" unit="count" value={l.deltaT} help="Em K (ou °C) em relação à atmosfera padrão." onChange={(v) => setL("deltaT", v)} />
      </div>
      <p className="note">Sem vento nesta versão (o vento entra na fase 4).</p>
    </section>
  );
}
