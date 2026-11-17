import { SAMPLE, type Design, type SimOutput } from "@astra/core-js";
import { bearingName } from "../geo";
import { fmt } from "../units";
import { sampleAt } from "./telemetry";

const COLORS = ["#38d6ff", "#ff9a3c", "#4ade80", "#c084fc", "#facc15", "#f472b6", "#94a3b8"];
const R_EARTH = 6_371_008.8;

/**
 * Mapa de recuperação visto de cima, no plano tangente ao local (norte para cima).
 * Funciona sem internet. Mostra trajetória no solo, apogeu, pouso de cada corpo com
 * distância e azimute, vento de superfície, zonas e o instante atual.
 */
export function RecoveryMap({ design, out, t }: { design: Design; out: SimOutput; t: number }) {
  const l = design.launch;
  const ne = (lat: number, lon: number): [number, number] => [
    ((lat - l.latDeg) * Math.PI * R_EARTH) / 180,
    ((lon - l.lonDeg) * Math.PI * R_EARTH * Math.cos((l.latDeg * Math.PI) / 180)) / 180,
  ];
  const pts = out.bodies.flatMap((b) => b.samples.map((r) => [r[SAMPLE.north]!, r[SAMPLE.east]!]));
  const zones = design.zones.map((z) => ({ ...z, c: ne(z.latDeg, z.lonDeg) }));
  let ext = 50;
  for (const [n, e] of pts) ext = Math.max(ext, Math.abs(n!), Math.abs(e!));
  for (const z of zones) ext = Math.max(ext, Math.abs(z.c[0]) + z.radius, Math.abs(z.c[1]) + z.radius);
  ext *= 1.15;
  const X = (e: number) => e;
  const Y = (n: number) => -n;
  const sw = ext / 250;
  // Barra de escala: potência de 10 (×1, 2 ou 5) próxima de 1/4 da largura.
  const target = ext / 2;
  const p10 = 10 ** Math.floor(Math.log10(target));
  const bar = [1, 2, 5, 10].map((k) => k * p10).filter((v) => v <= target).at(-1) ?? p10;
  const surface = l.wind.layers[0];
  const main = out.bodies[0];
  const apo = main?.samples.reduce((a, r) => (r[SAMPLE.alt]! > a[SAMPLE.alt]! ? r : a), main.samples[0]!);

  return (
    <section className="panel recovery">
      <h2>Mapa de recuperação</h2>
      <svg viewBox={`${-ext} ${-ext} ${2 * ext} ${2 * ext}`} role="img" aria-label="Mapa de recuperação visto de cima, norte para cima">
        <defs>
          <marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
            <path d="M0,0 L10,5 L0,10 z" fill="#94a3b8" />
          </marker>
        </defs>
        {[0.25, 0.5, 0.75, 1].map((k) => (
          <circle key={k} r={ext * k} className="ring" strokeWidth={sw * 0.6} />
        ))}
        <line x1={-ext} y1={0} x2={ext} y2={0} className="axis" strokeWidth={sw * 0.5} />
        <line x1={0} y1={-ext} x2={0} y2={ext} className="axis" strokeWidth={sw * 0.5} />
        {zones.map((z, i) => (
          <g key={i}>
            <circle cx={X(z.c[1])} cy={Y(z.c[0])} r={z.radius} className={`zone ${z.kind}`} strokeWidth={sw} />
            <text x={X(z.c[1])} y={Y(z.c[0]) - z.radius - sw * 3} className="lbl" fontSize={sw * 10} textAnchor="middle">{z.name}</text>
          </g>
        ))}
        {out.bodies.map((b, i) => (
          <polyline
            key={b.id}
            points={b.samples.map((r) => `${X(r[SAMPLE.east]!)},${Y(r[SAMPLE.north]!)}`).join(" ")}
            fill="none"
            stroke={COLORS[i % COLORS.length]}
            strokeWidth={sw * 1.4}
            strokeDasharray={i ? `${sw * 4} ${sw * 3}` : undefined}
          />
        ))}
        {surface && surface.speed > 0 && (
          <line
            x1={X(-ext * 0.85)}
            y1={Y(ext * 0.85)}
            x2={X(-ext * 0.85 - Math.sin((surface.fromDeg * Math.PI) / 180) * ext * 0.15)}
            y2={Y(ext * 0.85 - Math.cos((surface.fromDeg * Math.PI) / 180) * ext * 0.15)}
            stroke="#94a3b8"
            strokeWidth={sw * 2}
            markerEnd="url(#arrow)"
          />
        )}
        <circle r={sw * 4} className="pad" />
        {apo && (
          <g>
            <circle cx={X(apo[SAMPLE.east]!)} cy={Y(apo[SAMPLE.north]!)} r={sw * 3} className="apo" />
            <text x={X(apo[SAMPLE.east]!) + sw * 5} y={Y(apo[SAMPLE.north]!)} fontSize={sw * 10} className="lbl">apogeu {fmt(apo[SAMPLE.alt]!, 0)} m</text>
          </g>
        )}
        {out.bodies.map((b, i) => {
          if (!b.landing) return null;
          const [, , n, e] = b.landing;
          return (
            <g key={b.id}>
              <circle cx={X(e)} cy={Y(n)} r={sw * 4} fill={COLORS[i % COLORS.length]} stroke="#fff" strokeWidth={sw} />
              <text x={X(e) + sw * 6} y={Y(n) + sw * 4} fontSize={sw * 10} className="lbl">
                {b.name}: {fmt(Math.hypot(n, e), 0)} m a {bearingName(n, e).name}
              </text>
            </g>
          );
        })}
        {out.bodies.map((b, i) => {
          const r = sampleAt(b, t);
          return r ? <circle key={b.id} cx={X(r[SAMPLE.east]!)} cy={Y(r[SAMPLE.north]!)} r={sw * 3} fill="none" stroke={COLORS[i % COLORS.length]} strokeWidth={sw * 1.5} /> : null;
        })}
        <g transform={`translate(${ext * 0.85},${-ext * 0.8})`}>
          <path d={`M0,${-sw * 12} L${sw * 5},${sw * 4} L0,0 L${-sw * 5},${sw * 4} Z`} fill="#dbe7f5" />
          <text y={-sw * 15} fontSize={sw * 11} textAnchor="middle" className="lbl">N</text>
        </g>
        <g transform={`translate(${-ext * 0.9},${ext * 0.9})`}>
          <line x1={0} y1={0} x2={bar} y2={0} stroke="#dbe7f5" strokeWidth={sw * 1.5} />
          <text x={bar / 2} y={-sw * 4} fontSize={sw * 10} textAnchor="middle" className="lbl">{bar >= 1000 ? `${bar / 1000} km` : `${bar} m`}</text>
        </g>
      </svg>
      <ul className="landings">
        {out.bodies.filter((b) => b.landing).map((b) => {
          const [, v, n, e] = b.landing!;
          const br = bearingName(n, e);
          return (
            <li key={b.id}>
              <b>{b.name}</b> cairá a {Math.hypot(n, e) >= 10_000 ? `${fmt(Math.hypot(n, e) / 1000, 0)} km` : `${fmt(Math.hypot(n, e), 0)} m`} a {br.name} ({fmt(br.deg, 0)}°), a {fmt(v, 1)} m/s.
            </li>
          );
        })}
        {surface && <li>Vento de superfície: {fmt(surface.speed, 1)} m/s vindo de {fmt(surface.fromDeg, 0)}°.</li>}
      </ul>
      <p className="note">A elipse de dispersão dos pousos está na aba Análises (Monte Carlo).</p>
    </section>
  );
}
