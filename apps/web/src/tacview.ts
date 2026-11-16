/**
 * CSV do Tacview (formato "real-life CSV" da documentação do Tacview): exportação de cada
 * corpo do voo e leitura de registros de voo reais.
 *
 * Colunas: Time (s desde o início, ou UTC ISO), Longitude, Latitude (°), Altitude (m acima
 * do mar), Roll, Pitch, Yaw (°, guinada a partir do norte verdadeiro), AOA (°), TAS (m/s),
 * Mach. Nome do arquivo: "Objeto (piloto) [Cor].csv".
 */
import { SAMPLE, type SimOutput } from "@astra/core-js";
import { strToU8, zipSync } from "fflate";

const COLORS = ["Orange", "Blue", "Green", "Violet", "Yellow", "Red", "Cyan", "Grey"];
const n = (x: number, d: number) => (Number.isFinite(x) ? x.toFixed(d) : "");

/**
 * Arfagem e guinada do eixo do corpo (dado em ECEF) no referencial local leste-norte-cima
 * do ponto: arfagem = asin(eixo·cima), guinada = atan2(eixo·leste, eixo·norte).
 */
export function pitchYaw(axis: [number, number, number], latDeg: number, lonDeg: number): [number, number] {
  const [f, l] = [(latDeg * Math.PI) / 180, (lonDeg * Math.PI) / 180];
  const east = [-Math.sin(l), Math.cos(l), 0];
  const north = [-Math.sin(f) * Math.cos(l), -Math.sin(f) * Math.sin(l), Math.cos(f)];
  const up = [Math.cos(f) * Math.cos(l), Math.cos(f) * Math.sin(l), Math.sin(f)];
  const dot = (a: number[]) => a[0]! * axis[0] + a[1]! * axis[1] + a[2]! * axis[2];
  const pitch = (Math.asin(Math.max(-1, Math.min(1, dot(up)))) * 180) / Math.PI;
  const yaw = (((Math.atan2(dot(east), dot(north)) * 180) / Math.PI) % 360 + 360) % 360;
  return [pitch, yaw];
}

/** Um arquivo CSV do Tacview por corpo. */
export function toTacview(out: SimOutput, pilot: string): { name: string; text: string }[] {
  const safe = (s: string) => s.replace(/[()[\]\\/:*?"<>|]/g, " ").trim() || "ASTRA";
  return out.bodies.map((b, i) => {
    const rows = b.samples.map((r) => {
      const [pitch, yaw] = pitchYaw([r[SAMPLE.axisX]!, r[SAMPLE.axisY]!, r[SAMPLE.axisZ]!], r[SAMPLE.lat]!, r[SAMPLE.lon]!);
      return [n(r[SAMPLE.t]!, 3), n(r[SAMPLE.lon]!, 7), n(r[SAMPLE.lat]!, 7), n(r[SAMPLE.altMsl]!, 2), "", n(pitch, 2), n(yaw, 2), n(r[SAMPLE.alphaDeg]!, 2), n(r[SAMPLE.speed]!, 2), n(r[SAMPLE.mach]!, 3)].join(",");
    });
    return { name: `${safe(b.name)} (${safe(pilot)}) [${COLORS[i % COLORS.length]}].csv`, text: ["Time,Longitude,Latitude,Altitude,Roll,Pitch,Yaw,AOA,TAS,Mach", ...rows].join("\n") + "\n" };
  });
}

/** Arquivo para baixar: um CSV, ou um zip quando há vários corpos. */
export function tacviewFile(out: SimOutput, pilot: string): { name: string; data: Uint8Array; type: string } {
  const files = toTacview(out, pilot);
  if (files.length === 1) return { name: files[0]!.name, data: strToU8(files[0]!.text), type: "text/csv" };
  return { name: `${pilot.replace(/[^\w-]+/g, "_")}-tacview.zip`, data: zipSync(Object.fromEntries(files.map((f) => [f.name, strToU8(f.text)]))), type: "application/zip" };
}

export interface TacviewPoint {
  t: number;
  lat: number;
  lon: number;
  alt: number;
}

/** Divide uma linha CSV respeitando aspas. */
function cells(line: string, sep: string): string[] {
  const out: string[] = [];
  let cur = "";
  let q = false;
  for (const ch of line) {
    if (ch === '"') q = !q;
    else if (ch === sep && !q) {
      out.push(cur);
      cur = "";
    } else cur += ch;
  }
  out.push(cur);
  return out.map((c) => c.trim());
}

/**
 * Lê um CSV no formato do Tacview. Aceita Time em segundos ou UTC ISO, ou Timestamp Unix;
 * campos vazios repetem o valor anterior (regra do formato). Devolve null se o cabeçalho não
 * for do Tacview.
 */
export function parseTacview(text: string): TacviewPoint[] | null {
  const lines = text.replace(/^﻿/, "").split(/\r?\n/).filter((l) => l.trim());
  if (lines.length < 2) return null;
  const sep = [",", ";", "\t"].sort((a, b) => lines[0]!.split(b).length - lines[0]!.split(a).length)[0]!;
  const head = cells(lines[0]!, sep).map((h) => h.toLowerCase().replace(/\s*\(.*\)\s*$/, ""));
  const col = (k: string) => head.indexOf(k);
  const [ti, ts, lo, la, al] = [col("time"), col("timestamp"), col("longitude"), col("latitude"), col("altitude")];
  if ((ti < 0 && ts < 0) || lo < 0 || la < 0) return null;
  const pts: TacviewPoint[] = [];
  let prev = { lat: Number.NaN, lon: Number.NaN, alt: 0 };
  let t0: number | null = null;
  for (const line of lines.slice(1)) {
    const c = cells(line, sep);
    let t: number;
    if (ts >= 0 && c[ts]) t = Number(c[ts]);
    else {
      const raw = c[ti] ?? "";
      t = /^\d+(\.\d+)?$/.test(raw) ? Number(raw) : Date.parse(raw) / 1000;
    }
    if (!Number.isFinite(t)) continue;
    const num = (i: number, fallback: number) => (i >= 0 && c[i] !== undefined && c[i] !== "" ? Number(c[i]) : fallback);
    const p = { lat: num(la, prev.lat), lon: num(lo, prev.lon), alt: num(al, prev.alt) };
    prev = p;
    if (!Number.isFinite(p.lat) || !Number.isFinite(p.lon)) continue;
    t0 ??= t;
    pts.push({ t: t - t0, ...p });
  }
  return pts;
}
