import type { Design, Validation } from "@astra/core-js";
import { fmt } from "../units";

interface Props {
  design: Design;
  validation: Validation | null;
  selected: string | null;
  errorParts: Set<string>;
  onSelect: (id: string) => void;
}

/** Vista lateral em escala: casca externa, aletas, peças internas, CG e CP. */
export function RocketView({ design, validation, selected, errorParts, onSelect }: Props) {
  if (!validation || validation.summary.profile.length === 0) {
    return <div className="rocket-view empty">Adicione peças para ver o foguete.</div>;
  }
  const s = validation.summary;
  const x0 = s.profile[0]![0];
  const L = Math.max(s.length, 0.05);
  const R = Math.max(...s.profile.map((p) => p[1]), 0.005);
  const finSpan = Math.max(0, ...design.parts.map((p) => (p.kind.type === "finSet" ? p.kind.span : 0)));
  const H = 2 * (R + finSpan) * 1.15;
  const pad = 0.03 * L;
  const vb = `${x0 - pad} ${-H / 2} ${L + 2 * pad} ${H}`;
  const top = s.profile.map(([x, r]) => `${x},${-r}`).join(" ");
  const bottom = [...s.profile].reverse().map(([x, r]) => `${x},${r}`).join(" ");
  const stroke = L / 600;
  const cls = (id: string) => ["part", selected === id ? "sel" : "", errorParts.has(id) ? "bad" : ""].join(" ");
  const radiusAt = (x: number) => {
    const p = s.profile;
    for (let i = 1; i < p.length; i++) if (p[i]![0] >= x) return p[i]![1];
    return p.at(-1)![1];
  };

  return (
    <figure className="rocket-view">
      <svg viewBox={vb} preserveAspectRatio="xMidYMid meet" role="img" aria-label={`Vista lateral de ${design.name}`}>
        <polygon points={`${top} ${bottom}`} className="hull" strokeWidth={stroke} />
        {design.parts.map((p, i) => {
          const e = s.extents[i];
          if (!e) return null;
          const [a, b, rin, rout] = e;
          const k = p.kind;
          const common = { className: cls(p.id), strokeWidth: stroke, onClick: () => onSelect(p.id) };
          if (k.type === "finSet") {
            const r = radiusAt(p.position + k.rootChord / 2);
            const pts = (sgn: number) =>
              [
                [p.position, sgn * r],
                [p.position + k.sweep, sgn * (r + k.span)],
                [p.position + k.sweep + k.tipChord, sgn * (r + k.span)],
                [p.position + k.rootChord, sgn * r],
              ]
                .map((q) => q.join(","))
                .join(" ");
            return (
              <g key={p.id}>
                <polygon points={pts(-1)} {...common} />
                <polygon points={pts(1)} {...common} />
                <title>{p.name}</title>
              </g>
            );
          }
          if (k.type === "noseCone" || k.type === "bodyTube" || k.type === "transition") {
            const end = k.type === "noseCone" ? p.position + k.length : b;
            return (
              <rect key={p.id} x={p.position} y={-R} width={Math.max(end - p.position, 0)} height={2 * R} {...common} className={`${cls(p.id)} ext`}>
                <title>{p.name}</title>
              </rect>
            );
          }
          if (k.type === "coupler" || k.type === "motorMount" || k.type === "centeringRing") {
            return (
              <g key={p.id} onClick={common.onClick}>
                <rect x={a} y={-rout} width={b - a} height={rout - rin} className={cls(p.id)} strokeWidth={stroke} />
                <rect x={a} y={rin} width={b - a} height={rout - rin} className={cls(p.id)} strokeWidth={stroke} />
                <title>{p.name}</title>
              </g>
            );
          }
          if (k.type === "shockCord") {
            return (
              <circle key={p.id} cx={a} cy={0} r={R / 4} {...common} className={`${cls(p.id)} cord`}>
                <title>{p.name}</title>
              </circle>
            );
          }
          return (
            <rect key={p.id} x={a} y={-rout} width={Math.max(b - a, stroke)} height={2 * rout} {...common} className={`${cls(p.id)} ${k.type === "parachute" ? "chute" : "solid"}`}>
              <title>{p.name}</title>
            </rect>
          );
        })}
        <g className="marker cg" transform={`translate(${s.cg},0)`}>
          <circle r={R * 0.55} strokeWidth={stroke * 1.5} />
          <path d={`M0,${-R * 0.55} A${R * 0.55},${R * 0.55} 0 0 1 ${R * 0.55},0 L0,0 Z M0,${R * 0.55} A${R * 0.55},${R * 0.55} 0 0 1 ${-R * 0.55},0 L0,0 Z`} />
          <title>CG</title>
        </g>
        <g className="marker cp" transform={`translate(${s.cp},0)`}>
          <circle r={R * 0.55} strokeWidth={stroke * 1.5} />
          <circle r={R * 0.15} className="dot" />
          <title>CP</title>
        </g>
      </svg>
      <figcaption>
        <span className="legend cg">CG {fmt(s.cg * 1000, 0)} mm</span>
        <span className="legend cp">CP {fmt(s.cp * 1000, 0)} mm</span>
        <span>Margem {fmt(s.staticMargin, 2)} cal</span>
        <span>Comprimento {fmt(s.length * 1000, 0)} mm</span>
        <span>Ø {fmt(s.diameter * 1000, 1)} mm</span>
      </figcaption>
    </figure>
  );
}
