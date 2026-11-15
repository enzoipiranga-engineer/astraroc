/**
 * Serviços externos chamados pelo navegador (o núcleo não faz E/S): busca de locais,
 * altitude do terreno e perfil de vento previsto ou histórico, todos da Open-Meteo
 * (gratuita, sem chave). Fonte dos dados: modelos meteorológicos globais (GFS, ICON,
 * ECMWF) agregados pela Open-Meteo, https://open-meteo.com/en/docs.
 */
import type { WindLayer } from "@astra/core-js";

export interface Place {
  name: string;
  detail: string;
  lat: number;
  lon: number;
  elevation: number;
}

export async function searchPlaces(q: string): Promise<Place[]> {
  const url = `https://geocoding-api.open-meteo.com/v1/search?name=${encodeURIComponent(q)}&count=8&language=pt&format=json`;
  const r = await fetch(url);
  if (!r.ok) throw new Error(`busca de locais falhou (${r.status})`);
  const j = (await r.json()) as { results?: { name: string; admin1?: string; country?: string; latitude: number; longitude: number; elevation?: number }[] };
  return (j.results ?? []).map((x) => ({
    name: x.name,
    detail: [x.admin1, x.country].filter(Boolean).join(", "),
    lat: x.latitude,
    lon: x.longitude,
    elevation: x.elevation ?? 0,
  }));
}

export async function elevationAt(lat: number, lon: number): Promise<number> {
  const r = await fetch(`https://api.open-meteo.com/v1/elevation?latitude=${lat}&longitude=${lon}`);
  if (!r.ok) throw new Error(`altitude do terreno indisponível (${r.status})`);
  const j = (await r.json()) as { elevation?: number[] };
  return j.elevation?.[0] ?? 0;
}

/** Níveis de pressão consultados (hPa), do solo até ~12 km. */
const LEVELS = [1000, 975, 950, 925, 900, 850, 800, 700, 600, 500, 400, 300, 250, 200];
/** Alturas fixas próximas do solo (m) fornecidas pela API. */
const NEAR = [10, 80, 120, 180];

/**
 * Monta a URL da Open-Meteo para o instante `utc`. Até 92 dias atrás e 16 à frente usa a
 * previsão; antes disso, o arquivo de previsões históricas.
 */
export function windUrl(lat: number, lon: number, utc: Date): string {
  const day = utc.toISOString().slice(0, 10);
  const vars = [
    ...NEAR.flatMap((h) => [`wind_speed_${h}m`, `wind_direction_${h}m`]),
    ...LEVELS.flatMap((p) => [`wind_speed_${p}hPa`, `wind_direction_${p}hPa`, `geopotential_height_${p}hPa`]),
  ].join(",");
  const ageDays = (Date.now() - utc.getTime()) / 86_400_000;
  const host = ageDays > 90 ? "historical-forecast-api.open-meteo.com" : "api.open-meteo.com";
  return `https://${host}/v1/forecast?latitude=${lat}&longitude=${lon}&hourly=${vars}&wind_speed_unit=ms&timezone=UTC&start_date=${day}&end_date=${day}`;
}

type Hourly = Record<string, (number | null)[]> & { time: string[] };

/**
 * Converte a resposta horária da Open-Meteo no perfil do instante mais próximo de `utc`.
 * Alturas: as fixas são acima do solo; os níveis de pressão vêm em altura geopotencial
 * acima do nível do mar, da qual se subtrai a altitude do solo (níveis abaixo do solo
 * são descartados). A diferença entre altura geopotencial e geométrica (< 0,3% até
 * 12 km) é desprezada.
 */
export function profileFromResponse(hourly: Hourly, utc: Date, groundAltitude: number): WindLayer[] {
  const target = utc.toISOString().slice(0, 13);
  let i = hourly.time.findIndex((t) => t.slice(0, 13) === target);
  if (i < 0) i = 0;
  const layers: WindLayer[] = [];
  const val = (k: string) => hourly[k]?.[i];
  for (const h of NEAR) {
    const s = val(`wind_speed_${h}m`);
    const d = val(`wind_direction_${h}m`);
    if (s != null && d != null) layers.push({ agl: h, speed: s, fromDeg: d });
  }
  for (const p of LEVELS) {
    const s = val(`wind_speed_${p}hPa`);
    const d = val(`wind_direction_${p}hPa`);
    const z = val(`geopotential_height_${p}hPa`);
    if (s == null || d == null || z == null) continue;
    const agl = z - groundAltitude;
    if (agl > 200) layers.push({ agl, speed: s, fromDeg: d });
  }
  layers.sort((a, b) => a.agl - b.agl);
  if (layers.length && layers[0]!.agl > 0) layers.unshift({ ...layers[0]!, agl: 0 });
  return layers;
}

export async function fetchWind(lat: number, lon: number, utc: Date, ground: number): Promise<WindLayer[]> {
  const r = await fetch(windUrl(lat, lon, utc));
  const j = (await r.json()) as { hourly?: Hourly; reason?: string };
  if (!r.ok || !j.hourly) throw new Error(j.reason ?? `previsão indisponível (${r.status})`);
  const layers = profileFromResponse(j.hourly, utc, ground);
  if (!layers.length) throw new Error("a resposta não trouxe vento para essa data");
  return layers;
}

/** Lê um perfil CSV: altura (m), velocidade (m/s), direção de onde vem (graus). */
export function parseWindCsv(text: string): WindLayer[] {
  const out: WindLayer[] = [];
  for (const line of text.split(/\r?\n/)) {
    const cols = line.split(/[,;\t]/).map((c) => Number(c.trim().replace(",", ".")));
    if (cols.length >= 3 && cols.slice(0, 3).every(Number.isFinite)) out.push({ agl: cols[0]!, speed: cols[1]!, fromDeg: cols[2]! });
  }
  if (!out.length) throw new Error("nenhuma linha válida (esperado: altura, velocidade, direção)");
  return out.sort((a, b) => a.agl - b.agl);
}

/** Perfil de lei de potência v(h) = v_ref·(h/h_ref)^α (Hellmann). */
export function powerLawProfile(vRef: number, hRef: number, alpha: number, fromDeg: number, top: number): WindLayer[] {
  const out: WindLayer[] = [];
  for (let h = 2; h < top; h *= 1.6) out.push({ agl: Math.round(h), speed: vRef * (h / hRef) ** alpha, fromDeg });
  out.push({ agl: top, speed: vRef * (top / hRef) ** alpha, fromDeg });
  return out;
}

export interface TrackPoint {
  lat: number;
  lon: number;
  alt: number;
}

/** Lê uma trajetória de GPS real: GPX (trkpt), KML (coordinates) ou CSV (lat, lon, alt). */
export function parseTrack(name: string, text: string): TrackPoint[] {
  const lower = name.toLowerCase();
  if (lower.endsWith(".gpx")) {
    const pts: TrackPoint[] = [];
    const re = /<trkpt[^>]*lat="([-\d.]+)"[^>]*lon="([-\d.]+)"[^>]*>([\s\S]*?)<\/trkpt>/g;
    for (let m = re.exec(text); m; m = re.exec(text)) {
      const ele = /<ele>([-\d.]+)<\/ele>/.exec(m[3]!);
      pts.push({ lat: Number(m[1]), lon: Number(m[2]), alt: ele ? Number(ele[1]) : 0 });
    }
    return pts;
  }
  if (lower.endsWith(".kml")) {
    const block = /<coordinates>([\s\S]*?)<\/coordinates>/.exec(text)?.[1] ?? "";
    return block
      .trim()
      .split(/\s+/)
      .map((c) => c.split(",").map(Number))
      .filter((c) => c.length >= 2 && c.every(Number.isFinite))
      .map(([lon, lat, alt]) => ({ lat: lat!, lon: lon!, alt: alt ?? 0 }));
  }
  const pts: TrackPoint[] = [];
  for (const line of text.split(/\r?\n/)) {
    const c = line.split(/[,;\t]/).map((x) => Number(x.trim()));
    if (c.length >= 2 && Number.isFinite(c[0]) && Number.isFinite(c[1]) && Math.abs(c[0]!) <= 90) pts.push({ lat: c[0]!, lon: c[1]!, alt: Number.isFinite(c[2]) ? c[2]! : 0 });
  }
  return pts;
}

export const bearingName = (north: number, east: number) => {
  const deg = ((Math.atan2(east, north) * 180) / Math.PI + 360) % 360;
  const names = ["norte", "nordeste", "leste", "sudeste", "sul", "sudoeste", "oeste", "noroeste"];
  return { deg, name: names[Math.round(deg / 45) % 8]! };
};
