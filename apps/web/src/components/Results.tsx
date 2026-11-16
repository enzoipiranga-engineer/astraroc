import { useMemo } from "react";
import { SAMPLE, type SimOutput } from "@astra/core-js";
import { fmt } from "../units";
import { Chart, type Series } from "./Chart";

const COLORS = ["#38d6ff", "#ff9a3c", "#4ade80", "#c084fc", "#facc15", "#f472b6", "#94a3b8"];

const eventIcon: Record<string, string> = {
  railExit: "⇡",
  burnout: "◌",
  apogee: "▲",
  ejection: "✸",
  deploy: "☂",
  impact: "▼",
  jointBroken: "⚡",
  finLost: "⚡",
  buckled: "⚡",
  cordBroken: "⚡",
  ejectionFailed: "✕",
  noExit: "✕",
};

const bearing = (n: number, e: number) => {
  const deg = ((Math.atan2(e, n) * 180) / Math.PI + 360) % 360;
  const names = ["norte", "nordeste", "leste", "sudeste", "sul", "sudoeste", "oeste", "noroeste"];
  return names[Math.round(deg / 45) % 8];
};

function LauncherCompare({ r }: { r: NonNullable<SimOutput["launcher"]> }) {
  const over = r.maxAxialG > r.axialLimitG || r.maxLateralG > r.lateralLimitG;
  return (
    <div className="compare">
      <h3>Com e sem o sistema de lançamento ({r.name})</h3>
      <table className="bodies">
        <thead><tr><th /><th>Trilho comum</th><th>Com o lançador</th></tr></thead>
        <tbody>
          <tr><td>Apogeu</td><td className="num">{fmt(r.apogeeWithout, 0)} m</td><td className="num">{fmt(r.apogeeWith, 0)} m ({r.apogeeWith >= r.apogeeWithout ? "+" : ""}{fmt(r.apogeeWith - r.apogeeWithout, 0)})</td></tr>
          <tr><td>Liberação</td><td className="num">—</td><td className="num">{fmt(r.releaseSpeed, 1)} m/s a {fmt(r.releaseAltitude, 0)} m, aos {fmt(r.releaseTime, 1)} s</td></tr>
          <tr><td>Δv equivalente entregue</td><td className="num">—</td><td className="num">{fmt(r.dvEquivalent, 0)} m/s (≈ {fmt(r.propellantEquivalent, 2)} kg de propelente)</td></tr>
          <tr className={over ? "bad" : ""}><td>Cargas no foguete</td><td className="num">—</td><td className="num">{fmt(r.maxAxialG, 1)} g axial (limite {fmt(r.axialLimitG, 0)}), {fmt(r.maxLateralG, 1)} g lateral (limite {fmt(r.lateralLimitG, 0)})</td></tr>
          <tr><td>Energia do lançador</td><td className="num">—</td><td className="num">{fmt(r.energy.input / 1000, 1)} kJ entregues, {fmt(r.energy.dissipated / 1000, 1)} kJ dissipados; balanço fecha em {r.energy.residual.toExponential(1)}</td></tr>
        </tbody>
      </table>
    </div>
  );
}

export function Results({ out, onPick, onExport }: { out: SimOutput; onPick?: ((t: number) => void) | undefined; onExport?: ((kind: "kml" | "kmz" | "csv" | "tacview") => void) | undefined }) {
  const series = useMemo(() => {
    const pick = (col: number): Series[] =>
      out.bodies.map((b, i) => ({
        label: b.name,
        color: COLORS[i % COLORS.length]!,
        t: b.samples.map((r) => r[SAMPLE.t]!),
        y: b.samples.map((r) => r[col]!),
      }));
    return { alt: pick(SAMPLE.alt), speed: pick(SAMPLE.speed), alpha: pick(SAMPLE.alphaDeg).filter((s) => s.y.some(Number.isFinite)) };
  }, [out]);
  const main = out.bodies[0];
  const apogee = main ? Math.max(...main.samples.map((r) => r[SAMPLE.alt]!)) : 0;
  const vmax = main ? Math.max(...main.samples.map((r) => r[SAMPLE.speed]!)) : 0;
  const mmax = main ? Math.max(...main.samples.map((r) => r[SAMPLE.mach]!)) : 0;
  const failures = out.events.filter((e) => ["jointBroken", "finLost", "buckled", "cordBroken", "ejectionFailed", "noExit"].includes(e.kind));

  return (
    <section className="panel results">
      <div className="results-head">
        <h2>Resultado do voo</h2>
        {onExport && (
          <div className="tools">
            <button onClick={() => onExport("kml")}>KML (Google Earth)</button>
            <button onClick={() => onExport("kmz")}>KMZ</button>
            <button onClick={() => onExport("csv")}>Telemetria CSV</button>
            <button onClick={() => onExport("tacview")} title="CSV no formato do Tacview, um por corpo">Tacview</button>
          </div>
        )}
      </div>
      {onPick && <p className="note">Clique num gráfico para levar o voo 3D até aquele instante.</p>}
      {failures.length > 0 && (
        <p className="failure-banner">
          {failures.length === 1 ? "Houve uma falha física" : `Houve ${failures.length} falhas físicas`} no voo. Veja a linha do tempo.
        </p>
      )}
      <div className="cards">
        <div className="card"><b>{fmt(apogee, 0)} m</b><span>apogeu acima do solo</span></div>
        <div className="card"><b>{fmt(vmax, 0)} m/s</b><span>velocidade máxima (Mach {fmt(mmax, 2)})</span></div>
        <div className="card"><b>{fmt(Math.max(...out.bodies.map((b) => b.landing ? Math.hypot(b.landing[2], b.landing[3]) : 0)), 0)} m</b><span>maior distância de pouso</span></div>
        <div className="card"><b>{out.bodies.length}</b><span>{out.bodies.length === 1 ? "corpo" : "corpos depois das separações"}</span></div>
      </div>
      {out.launcher && <LauncherCompare r={out.launcher} />}
      <table className="bodies">
        <thead>
          <tr><th>Corpo</th><th>Modelo</th><th>Pouso</th><th>Destino</th></tr>
        </thead>
        <tbody>
          {out.bodies.map((b, i) => (
            <tr key={b.id}>
              <td><span className="swatch" style={{ background: COLORS[i % COLORS.length] }} />{b.name}</td>
              <td>{b.models.join(" → ")}</td>
              <td>{b.landing ? `${fmt(Math.hypot(b.landing[2], b.landing[3]), 0)} m a ${bearing(b.landing[2], b.landing[3])}, ${fmt(b.landing[0], 1)} s` : "—"}</td>
              <td>{b.fate}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <Chart title="Altura acima do solo" unit="m" series={series.alt} cursorSync="voo" onPick={onPick} />
      <Chart title="Velocidade" unit="m/s" series={series.speed} cursorSync="voo" onPick={onPick} />
      {series.alpha.length > 0 && <Chart title="Ângulo de ataque (6-DOF)" unit="graus" series={series.alpha} cursorSync="voo" onPick={onPick} />}
      <h3>Linha do tempo</h3>
      <ol className="timeline">
        {out.events.map((e, k) => (
          <li key={k} className={failures.includes(e) ? "fail" : ""}>
            <span className="t">{fmt(e.t, 2)} s</span>
            <span className="ic">{eventIcon[e.kind] ?? "·"}</span>
            <span className="who">{out.bodies[e.body]?.name}</span>
            <span>{e.message}</span>
          </li>
        ))}
      </ol>
      {out.warnings.map((w, k) => (
        <p key={k} className="warn">{w}</p>
      ))}
    </section>
  );
}
