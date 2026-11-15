import { useState } from "react";
import type { Design, Zone } from "@astra/core-js";
import { elevationAt, searchPlaces, type Place } from "../geo";
import { NumberField, SelectField, TextField } from "./Field";

/** Locais pré-definidos (coordenadas aproximadas). */
export const SITES = [
  { id: "sp", name: "São Paulo (SP)", lat: -23.55, lon: -46.63, alt: 760 },
  { id: "alcantara", name: "Alcântara (MA), aprox.", lat: -2.32, lon: -44.37, alt: 40 },
  { id: "kourou", name: "Kourou, aprox.", lat: 5.24, lon: -52.77, alt: 10 },
  { id: "canaveral", name: "Cabo Canaveral, aprox.", lat: 28.49, lon: -80.58, alt: 3 },
  { id: "starbase", name: "Starbase, aprox.", lat: 25.997, lon: -97.157, alt: 5 },
  { id: "vandenberg", name: "Vandenberg, aprox.", lat: 34.632, lon: -120.611, alt: 100 },
  { id: "baikonur", name: "Baikonur, aprox.", lat: 45.92, lon: 63.342, alt: 90 },
  { id: "jiuquan", name: "Jiuquan, aprox.", lat: 40.96, lon: 100.29, alt: 1000 },
  { id: "spaceport", name: "Spaceport America", lat: 32.99, lon: -106.975, alt: 1400 },
  { id: "blackrock", name: "Black Rock Desert, aprox.", lat: 40.9, lon: -119.06, alt: 1190 },
  { id: "esrange", name: "Esrange, aprox.", lat: 67.89, lon: 21.11, alt: 330 },
] as const;

interface Props {
  design: Design;
  onChange: (d: Design) => void;
  picking: boolean;
  onTogglePick: () => void;
  ionToken: string;
  onIonToken: (t: string) => void;
}

export function SitePanel({ design, onChange, picking, onTogglePick, ionToken, onIonToken }: Props) {
  const l = design.launch;
  const setL = (patch: Partial<Design["launch"]>) => onChange({ ...design, launch: { ...l, ...patch } });
  const [q, setQ] = useState("");
  const [results, setResults] = useState<Place[]>([]);
  const [msg, setMsg] = useState<string | null>(null);
  const [token, setToken] = useState(ionToken);
  const site = SITES.find((s) => Math.abs(s.lat - l.latDeg) < 1e-6 && Math.abs(s.lon - l.lonDeg) < 1e-6)?.id ?? "";

  const search = async () => {
    setMsg(null);
    try {
      const r = await searchPlaces(q);
      setResults(r);
      if (!r.length) setMsg("Nenhum local encontrado.");
    } catch (e) {
      setMsg((e as Error).message);
    }
  };
  const fixElevation = async () => {
    try {
      setL({ altitude: Math.round(await elevationAt(l.latDeg, l.lonDeg)) });
    } catch (e) {
      setMsg((e as Error).message);
    }
  };
  const setZone = (i: number, patch: Partial<Zone>) => onChange({ ...design, zones: design.zones.map((z, j) => (j === i ? { ...z, ...patch } : z)) });

  const utcLocal = l.utc ? l.utc.slice(0, 16) : "";
  return (
    <section className="panel site">
      <h2>Local, data e zonas</h2>
      <SelectField
        label="Local pré-definido"
        value={site}
        options={[{ value: "", label: "personalizado" }, ...SITES.map((s) => ({ value: s.id, label: s.name }))]}
        onChange={(id) => {
          const s = SITES.find((x) => x.id === id);
          if (s) setL({ latDeg: s.lat, lonDeg: s.lon, altitude: s.alt });
        }}
      />
      <div className="search">
        <input value={q} placeholder="Buscar cidade ou local…" onChange={(e) => setQ(e.target.value)} onKeyDown={(e) => e.key === "Enter" && void search()} aria-label="Buscar local" />
        <button onClick={() => void search()} disabled={!q.trim()}>Buscar</button>
        <button className={picking ? "on" : ""} onClick={onTogglePick} title="Clique no globo do voo 3D para escolher">No mapa</button>
      </div>
      {results.length > 0 && (
        <ul className="places">
          {results.map((p, i) => (
            <li key={i}>
              <button onClick={() => { setL({ latDeg: p.lat, lonDeg: p.lon, altitude: Math.round(p.elevation) }); setResults([]); }}>
                {p.name} <small>{p.detail} · {p.lat.toFixed(3)}, {p.lon.toFixed(3)} · {Math.round(p.elevation)} m</small>
              </button>
            </li>
          ))}
        </ul>
      )}
      {msg && <p className="hint">{msg}</p>}
      <div className="row">
        <NumberField label="Latitude" unit="deg" value={l.latDeg} onChange={(v) => v !== null && setL({ latDeg: v })} />
        <NumberField label="Longitude" unit="deg" value={l.lonDeg} onChange={(v) => v !== null && setL({ lonDeg: v })} />
      </div>
      <div className="row">
        <NumberField label="Altitude do solo" unit="m" value={l.altitude} onChange={(v) => v !== null && setL({ altitude: v })} />
        <button className="align-end" onClick={() => void fixElevation()}>Altitude do terreno</button>
      </div>
      <label className="field">
        <span>Data e hora do lançamento (UTC)</span>
        <input type="datetime-local" value={utcLocal} onChange={(e) => setL({ utc: e.target.value ? `${e.target.value}:00Z` : null })} />
      </label>
      <p className="hint">A data define a rotação da Terra, a iluminação do globo e o vento real.</p>

      <h3>Zonas</h3>
      {design.zones.map((z, i) => (
        <div key={i} className="zone-row">
          <TextField label="Nome" value={z.name} onChange={(v) => setZone(i, { name: v })} />
          <SelectField label="Tipo" value={z.kind} options={[{ value: "keepOut", label: "Proibida" }, { value: "landing", label: "Pouso" }]} onChange={(v) => setZone(i, { kind: v })} />
          <div className="row">
            <NumberField label="Latitude" unit="deg" value={z.latDeg} onChange={(v) => v !== null && setZone(i, { latDeg: v })} />
            <NumberField label="Longitude" unit="deg" value={z.lonDeg} onChange={(v) => v !== null && setZone(i, { lonDeg: v })} />
          </div>
          <NumberField label="Raio" unit="m" value={z.radius} onChange={(v) => v !== null && setZone(i, { radius: v })} />
          <button className="danger" onClick={() => onChange({ ...design, zones: design.zones.filter((_, j) => j !== i) })}>Remover zona</button>
        </div>
      ))}
      <button onClick={() => onChange({ ...design, zones: [...design.zones, { name: `Zona ${design.zones.length + 1}`, latDeg: l.latDeg + 0.002, lonDeg: l.lonDeg + 0.002, radius: 100, kind: "keepOut" }] })}>+ zona</button>

      <details className="ion">
        <summary>Mapas em alta resolução (Cesium ion)</summary>
        <p className="hint">
          Com uma chave gratuita de ion.cesium.com o globo ganha terreno 3D e imagem de satélite. A chave fica só neste navegador; não vai para o projeto.
        </p>
        <div className="search">
          <input type="password" value={token} onChange={(e) => setToken(e.target.value)} placeholder="chave do Cesium ion" aria-label="Chave do Cesium ion" />
          <button onClick={() => onIonToken(token.trim())}>Usar</button>
        </div>
      </details>
    </section>
  );
}
