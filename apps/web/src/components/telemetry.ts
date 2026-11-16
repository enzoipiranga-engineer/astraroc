/** Interpolação da telemetria num instante e cores por grandeza. */
import { SAMPLE, type BodyTrack } from "@astra/core-js";

export function sampleAt(b: BodyTrack, t: number): number[] | null {
  const s = b.samples;
  if (!s.length || t < s[0]![0]! || t > s.at(-1)![0]!) return null;
  let lo = 0;
  let hi = s.length - 1;
  while (hi - lo > 1) {
    const mid = (lo + hi) >> 1;
    if (s[mid]![0]! <= t) lo = mid;
    else hi = mid;
  }
  const a = s[lo]!;
  const c = s[hi]!;
  const w = c[0]! > a[0]! ? (t - a[0]!) / (c[0]! - a[0]!) : 0;
  // Colunas sem valor chegam do JSON como null (NaN no núcleo): continuam NaN.
  return a.map((v, i) => (v == null || c[i] == null ? NaN : v + (c[i]! - v) * w));
}

/** Aceleração específica aproximada (g), pela derivada da velocidade em relação ao ar. */
export function accelAt(b: BodyTrack, t: number): number {
  const h = 0.05;
  const t0 = b.samples[0]?.[0] ?? 0;
  const t1 = b.samples.at(-1)?.[0] ?? 0;
  const a = Math.max(t0, t - h);
  const c = Math.min(t1, t + h);
  const p = sampleAt(b, a);
  const n = sampleAt(b, c);
  if (!p || !n || c <= a) return NaN;
  return (n[SAMPLE.speed]! - p[SAMPLE.speed]!) / (c - a) / 9.80665;
}

export type ColorBy = "speed" | "mach" | "q" | "accel";

/** Escala de cor (azul → ciano → amarelo → laranja → vermelho) para v ∈ [0, 1]. */
export function ramp(v: number): [number, number, number] {
  const stops: [number, number, number][] = [
    [0.15, 0.35, 0.95],
    [0.22, 0.84, 1.0],
    [0.98, 0.9, 0.3],
    [1.0, 0.6, 0.24],
    [1.0, 0.25, 0.25],
  ];
  const x = Math.min(Math.max(v, 0), 1) * (stops.length - 1);
  const i = Math.min(Math.floor(x), stops.length - 2);
  const f = x - i;
  const a = stops[i]!;
  const b = stops[i + 1]!;
  return [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f, a[2] + (b[2] - a[2]) * f];
}

export function colorValue(b: BodyTrack, k: number, by: ColorBy): number {
  const r = b.samples[k]!;
  if (by === "speed") return r[SAMPLE.speed]!;
  if (by === "mach") return r[SAMPLE.mach]!;
  if (by === "q") return r[SAMPLE.q]!;
  const p = b.samples[Math.max(0, k - 1)]!;
  const n = b.samples[Math.min(b.samples.length - 1, k + 1)]!;
  return n[0]! > p[0]! ? Math.abs((n[SAMPLE.speed]! - p[SAMPLE.speed]!) / (n[0]! - p[0]!) / 9.80665) : 0;
}
