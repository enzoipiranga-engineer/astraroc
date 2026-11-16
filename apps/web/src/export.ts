/** Exportação da trajetória para KML (Google Earth) e da telemetria para CSV. */
import { SAMPLE, type Design, type SimOutput } from "@astra/core-js";

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
const COLORS = ["ffffd638", "ff3c9aff", "ff80de4a", "fffc84c0", "ff15ccfa"];

export function toKml(design: Design, out: SimOutput): string {
  const lines = out.bodies
    .map((b, i) => {
      const coords = b.samples.map((r) => `${r[SAMPLE.lon]!.toFixed(7)},${r[SAMPLE.lat]!.toFixed(7)},${r[SAMPLE.altMsl]!.toFixed(1)}`).join(" ");
      const land = b.samples.at(-1);
      return `
  <Style id="c${i}"><LineStyle><color>${COLORS[i % COLORS.length]}</color><width>3</width></LineStyle></Style>
  <Placemark><name>${esc(b.name)}</name><styleUrl>#c${i}</styleUrl>
    <LineString><altitudeMode>absolute</altitudeMode><coordinates>${coords}</coordinates></LineString></Placemark>${
      land && b.landing
        ? `
  <Placemark><name>${esc(b.name)}: pouso</name><description>${esc(b.fate)}</description><Point><coordinates>${land[SAMPLE.lon]},${land[SAMPLE.lat]},0</coordinates></Point></Placemark>`
        : ""
    }`;
    })
    .join("");
  const events = out.events
    .map((e) => {
      const b = out.bodies[e.body];
      const r = b?.samples.reduce((best, x) => (Math.abs(x[0]! - e.t) < Math.abs(best[0]! - e.t) ? x : best), b.samples[0]!);
      return r
        ? `
  <Placemark><name>${esc(`${e.t.toFixed(1)} s ${e.kind}`)}</name><description>${esc(e.message)}</description><Point><altitudeMode>absolute</altitudeMode><coordinates>${r[SAMPLE.lon]},${r[SAMPLE.lat]},${r[SAMPLE.altMsl]}</coordinates></Point></Placemark>`
        : "";
    })
    .join("");
  const l = design.launch;
  return `<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2"><Document>
  <name>${esc(design.name)}</name>
  <description>Simulado no ASTRA. Ferramenta de estudo; não substitui análise de segurança.</description>
  <Placemark><name>Lançamento</name><Point><coordinates>${l.lonDeg},${l.latDeg},0</coordinates></Point></Placemark>${lines}${events}
</Document></kml>
`;
}

export function toCsv(out: SimOutput): string {
  const head = "corpo,t_s,altura_agl_m,velocidade_ar_m_s,velocidade_vertical_m_s,mach,q_Pa,norte_m,leste_m,alfa_graus,massa_kg,lat_graus,lon_graus,altitude_m";
  const rows = out.bodies.flatMap((b) => b.samples.map((r) => [JSON.stringify(b.name), ...r.slice(0, 13).map((x) => (Number.isFinite(x) ? +x.toPrecision(9) : ""))].join(",")));
  return [head, ...rows].join("\n") + "\n";
}

export function download(name: string, text: string, type: string) {
  const url = URL.createObjectURL(new Blob([text], { type }));
  const a = Object.assign(document.createElement("a"), { href: url, download: name });
  a.click();
  URL.revokeObjectURL(url);
}

/** Nome de arquivo seguro a partir do nome do projeto (sem acentos nem símbolos). */
export function fileStem(name: string): string {
  return name.normalize("NFD").replace(/[̀-ͯ]/g, "").replace(/[^\w-]+/g, "_").replace(/^_+|_+$/g, "") || "projeto";
}
