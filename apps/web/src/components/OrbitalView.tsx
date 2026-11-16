import { lazy, Suspense, useState } from "react";
import type { CoreClient, Design, MissionOutput, MissionRequest, Validation } from "@astra/core-js";
import { fmt } from "../units";
import { NumberField, SelectField } from "./Field";
import { Results } from "./Results";

const GlobeView = lazy(() => import("./GlobeView").then((m) => ({ default: m.GlobeView })));

const SITES = [
  { id: "canaveral", name: "Cabo Canaveral (LC-39A), aprox.", lat: 28.6082, lon: -80.6041, alt: 3, az: 90 },
  { id: "alcantara", name: "Alcântara, aprox.", lat: -2.3167, lon: -44.3667, alt: 40, az: 90 },
  { id: "kourou", name: "Kourou, aprox.", lat: 5.239, lon: -52.768, alt: 10, az: 90 },
  { id: "starbase", name: "Starbase, aprox.", lat: 25.997, lon: -97.157, alt: 5, az: 95 },
  { id: "vandenberg", name: "Vandenberg (polar), aprox.", lat: 34.632, lon: -120.611, alt: 100, az: 196 },
  { id: "baikonur", name: "Baikonur, aprox.", lat: 45.92, lon: 63.342, alt: 90, az: 64 },
] as const;

interface Props {
  core: CoreClient;
  ionToken: string;
}

/** Lançador orbital: Falcon 9 ou Starship até uma órbita circular, com guiagem automática. */
export function OrbitalView({ core, ionToken }: Props) {
  const [launchCost, setLaunchCost] = useState<number | null>(null);
  const [req, setReq] = useState<MissionRequest>({
    vehicle: "falcon9",
    payload: 15_000,
    targetAltitude: 200_000,
    azimuthDeg: 90,
    latDeg: SITES[0].lat,
    lonDeg: SITES[0].lon,
    altitude: SITES[0].alt,
    utc: "2026-09-03T14:00:00Z",
    engineOut: null,
    kickDeg: null,
    recoveryReserve: 0,
  });
  const [out, setOut] = useState<MissionOutput | null>(null);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [seek, setSeek] = useState<{ t: number; n: number } | null>(null);
  const set = (p: Partial<MissionRequest>) => setReq({ ...req, ...p });
  const site = SITES.find((s) => s.lat === req.latDeg && s.lon === req.lonDeg)?.id ?? "";

  const run = async () => {
    setBusy(true);
    setErr(null);
    try {
      setOut(await core.mission(req));
    } catch (e) {
      setErr((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  // O globo usa o formato de projeto: monta um mínimo com o local da missão.
  const design = {
    name: req.vehicle,
    parts: [],
    zones: [],
    launch: { latDeg: req.latDeg, lonDeg: req.lonDeg, altitude: req.altitude, railLength: 0, railElevationDeg: 90, railAzimuthDeg: req.azimuthDeg, deltaT: 0, utc: req.utc ?? null, wind: { layers: [] } },
  } as unknown as Design;
  const size = (req.vehicle === "starship" ? { length: 121, diameter: 9 } : { length: 70, diameter: 3.66 }) as unknown;
  const validation = { summary: size, issues: [] } as unknown as Validation;
  const o = out?.orbit;
  const dv = out?.dv;

  return (
    <div className="orbital">
      <section className="panel">
        <h2>Lançador orbital</h2>
        <div className="orbital-form">
          <SelectField label="Lançador" value={req.vehicle} options={[{ value: "falcon9", label: "Falcon 9 (descartável)" }, { value: "starship", label: "Starship + Super Heavy" }]} onChange={(v) => set({ vehicle: v, payload: v === "starship" ? 100_000 : 15_000 })} />
          <NumberField label="Carga útil" unit="kg" value={req.payload} onChange={(v) => v !== null && set({ payload: v })} />
          <NumberField label="Custo do lançamento (US$)" unit="count" optional placeholder="informe" value={launchCost} help="Preço do lançamento ou custo do veículo, em dólares. Usado para o custo por kg em órbita." onChange={setLaunchCost} />
          <NumberField label="Órbita circular alvo" unit="m" value={req.targetAltitude} help="Altitude, m" onChange={(v) => v !== null && set({ targetAltitude: v })} />
          <SelectField
            label="Local"
            value={site}
            options={[{ value: "", label: "personalizado" }, ...SITES.map((s) => ({ value: s.id, label: s.name }))]}
            onChange={(id) => {
              const s = SITES.find((x) => x.id === id);
              if (s) set({ latDeg: s.lat, lonDeg: s.lon, altitude: s.alt, azimuthDeg: s.az });
            }}
          />
          <NumberField label="Azimute de lançamento" unit="deg" value={req.azimuthDeg} help="90° = leste (aproveita a rotação da Terra). A inclinação resulta de latitude e azimute." onChange={(v) => v !== null && set({ azimuthDeg: v })} />
          <NumberField label="Reserva para recuperação do 1º estágio" unit="ratio" value={req.recoveryReserve} help="Fração do propelente guardada para pousar o 1º estágio (0 a 0,3). A fase 10 simula o pouso." onChange={(v) => v !== null && set({ recoveryReserve: Math.min(0.3, Math.max(0, v)) })} />
          <label className="field check">
            <input type="checkbox" checked={!!req.engineOut} onChange={(e) => set({ engineOut: e.target.checked ? { stage: 0, after: 30, engines: 1 } : null })} />
            <span>Falha de motor no 1º estágio</span>
          </label>
          {req.engineOut && (
            <div className="row">
              <NumberField label="Aos" unit="s" value={req.engineOut.after} onChange={(v) => v !== null && set({ engineOut: { ...req.engineOut!, after: v } })} />
              <NumberField label="Motores perdidos" unit="count" value={req.engineOut.engines} onChange={(v) => v !== null && set({ engineOut: { ...req.engineOut!, engines: Math.max(1, Math.round(v)) } })} />
            </div>
          )}
        </div>
        <button className="launch" onClick={() => void run()} disabled={busy}>{busy ? "Calculando guiagem…" : "LANÇAR"}</button>
        {err && <p className="bad">{err}</p>}
        <p className="note">
          Massas e motores com números públicos aproximados (confiabilidade “estimado”). Guiagem: subida vertical, manobra inicial escolhida
          automaticamente, gravity turn e tangente linear no estágio superior ajustada para chegar à órbita.
        </p>
      </section>

      {out && o && dv && (
        <>
          <section className="panel">
            <h2>{out.reachedOrbit ? "Em órbita" : "Não atingiu a órbita"}</h2>
            <div className="cards">
              <div className="card"><b>{fmt(o.perigeeAlt / 1000, 0)} × {Number.isFinite(o.apogeeAlt) ? fmt(o.apogeeAlt / 1000, 0) : "∞"} km</b><span>perigeu × apogeu</span></div>
              <div className="card"><b>{fmt(o.incDeg, 2)}°</b><span>inclinação</span></div>
              <div className="card"><b>{fmt(out.propellantMargin / 1000, 1)} t</b><span>propelente sobrando no corte</span></div>
              <div className="card"><b>{fmt(out.liftoffMass / 1000, 0)} t</b><span>massa na decolagem</span></div>
              <div className="card"><b>{fmt(out.maxQ[0] / 1000, 1)} kPa</b><span>max-Q aos {fmt(out.maxQ[1], 0)} s</span></div>
              <div className="card"><b>{fmt(out.maxAccelG, 1)} g</b><span>aceleração máxima</span></div>
              {launchCost !== null && out.reachedOrbit && <div className="card"><b>US$ {fmt(launchCost / req.payload, 0)}</b><span>por kg em órbita</span></div>}
            </div>
            <h3>Orçamento de Δv</h3>
            <table className="bodies">
              <tbody>
                <tr><td>Δv ideal (Tsiolkovsky, vácuo)</td><td className="num">{fmt(dv.ideal, 0)} m/s</td></tr>
                <tr><td>Entregue pelos motores (∫T/m)</td><td className="num">{fmt(dv.delivered, 0)} m/s</td></tr>
                <tr><td>Perda gravitacional</td><td className="num">−{fmt(dv.gravityLoss, 0)} m/s</td></tr>
                <tr><td>Perda por arrasto</td><td className="num">−{fmt(dv.dragLoss, 0)} m/s</td></tr>
                <tr><td>Perda por direção do empuxo</td><td className="num">−{fmt(dv.steeringLoss, 0)} m/s</td></tr>
                <tr><td>Perda por contrapressão no bocal (já fora do entregue)</td><td className="num">{fmt(dv.pressureLoss, 0)} m/s</td></tr>
                <tr><td>Ganho de velocidade inercial</td><td className="num">{fmt(dv.achieved, 0)} m/s</td></tr>
                <tr><td>Ajuda da rotação da Terra</td><td className="num">+{fmt(dv.earthRotation, 0)} m/s</td></tr>
              </tbody>
            </table>
            <p className="note">Manobra inicial de {fmt(out.kickDeg, 1)}°. {out.guidance ? `Guiagem tan θ = ${fmt(out.guidance[0], 3)} − ${fmt(out.guidance[1], 5)}·τ ${out.guidance[2] ? "(convergiu)" : "(não convergiu)"}.` : ""}</p>
          </section>
          <div className="flight-view single">
            <Suspense fallback={<section className="panel globe">Carregando o globo 3D…</section>}>
              <GlobeView design={design} validation={validation} out={out} gps={[]} ionToken={ionToken} seek={seek} onTime={() => undefined} pickMode={false} onPickLocation={() => undefined} />
            </Suspense>
          </div>
          <Results out={out} onPick={(t) => setSeek((s) => ({ t, n: (s?.n ?? 0) + 1 }))} />
        </>
      )}
    </div>
  );
}
