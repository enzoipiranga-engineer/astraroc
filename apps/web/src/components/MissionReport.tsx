import type { CompetitionReport, PayloadReport } from "@astra/core-js";
import { Chart } from "./Chart";

const num = (v: number | null, d = 1) => (v === null || !Number.isFinite(v) ? "—" : v.toFixed(d));

/** Relatório de ambiente das cargas úteis e de pontuação, depois do voo. */
export function MissionReport({ payloads, competition }: { payloads: PayloadReport[]; competition: CompetitionReport | null | undefined }) {
  if (!payloads.length && !competition) return null;
  return (
    <section className="panel mission-report">
      <h2>Carga útil e competição</h2>
      {competition && <Competition c={competition} />}
      {payloads.map((p) => (
        <Env key={p.id} p={p} />
      ))}
    </section>
  );
}

function Competition({ c }: { c: CompetitionReport }) {
  return (
    <div className="comp">
      <h3>
        {c.name} <small>{c.document}</small>
      </h3>
      <p className={`verdict ${c.launchAllowed ? "ok" : "bad"}`}>
        {c.launchAllowed ? "Pode lançar" : "NÃO pode lançar: requisito de segurança violado"} · apogeu {num(c.apogee, 0)} m{c.target ? ` (alvo ${num(c.target, 0)} m, erro ${num((100 * (c.apogee - c.target)) / c.target, 1)} %)` : ""}
        {c.podiumEligible !== null && (c.podiumEligible ? " · disputa o pódio" : " · abaixo da metade do alvo: fora do pódio")}
      </p>
      {(c.apogeePoints !== null || c.recoveryPoints !== null) && (
        <table className="score">
          <tbody>
            {c.apogeePoints !== null && (
              <tr>
                <td>Apogeu</td>
                <td>{num(c.apogeePoints, 1)}</td>
              </tr>
            )}
            {c.recoveryPoints !== null && (
              <tr>
                <td>Recuperação (sem dano)</td>
                <td>{num(c.recoveryPoints, 0)}</td>
              </tr>
            )}
            {c.bonuses.map(([t, p]) => (
              <tr key={t}>
                <td>Bônus: {t}</td>
                <td>+{p}</td>
              </tr>
            ))}
            {c.penalties > 0 && (
              <tr className="bad">
                <td>Penalidades</td>
                <td>−{num(c.penalties, 0)}</td>
              </tr>
            )}
            <tr className="total">
              <td>Pontos de voo calculados</td>
              <td>
                {num(c.flightScore, 1)} / {num(c.flightMax, 0)}
              </td>
            </tr>
            {c.judged.map(([t, p]) => (
              <tr key={t} className="muted">
                <td>{t} (banca)</td>
                <td>até {p}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {c.results.map((r) => (
        <p key={r.id} className={`pcheck ${r.pass ? "ok" : "bad"}`}>
          {r.pass ? "✓" : "✗"} {r.text}: {num(r.value, 2)}
          {r.penalty > 0 && ` (−${r.penalty})`} <small>{r.source}</small>
        </p>
      ))}
      {c.pending.map((p) => (
        <p key={p} className="hint">Pendente: {p}</p>
      ))}
    </div>
  );
}

function Env({ p }: { p: PayloadReport }) {
  const t = p.rows.map((r) => r[0]);
  const col = (i: number) => p.rows.map((r) => r[i]!);
  return (
    <div className="env">
      <h3>
        {p.name} <span className={`badge ${p.pass ? "ok" : "bad"}`}>{p.pass ? "APROVADA" : "REPROVADA"}</span>
      </h3>
      <table className="checks">
        <thead>
          <tr>
            <th>Grandeza</th>
            <th>Voo</th>
            <th>Limite</th>
            <th>Margem</th>
          </tr>
        </thead>
        <tbody>
          {p.checks.map((c) => (
            <tr key={c.name} className={c.pass === false ? "bad" : c.pass ? "ok" : ""} title={c.note}>
              <td>{c.name}</td>
              <td>
                {num(c.value, c.unit === "Pa/s" || c.unit === "Pa" ? 0 : 1)} {c.unit}
              </td>
              <td>{c.limit === null ? "—" : `${num(c.limit, 1)} ${c.unit}`}</td>
              <td>{c.margin === null ? "—" : `${num(100 * c.margin, 0)} %`}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <p className="hint">
        Baia de {num(p.vent.volume * 1e6, 0)} cm³: para manter ΔP abaixo de {num(p.vent.dpLimit, 0)} Pa são precisos {num(p.vent.areaRequired * 1e6, 2)} mm² de respiro — 3 furos de {num(p.vent.diameterFor3 * 1000, 2)} mm (hoje: {num(p.vent.areaNow * 1e6, 2)} mm²).
      </p>
      <div className="charts">
        <Chart title="Aceleração na carga" unit="g" cursorSync="env" series={[{ label: "axial", color: "#ff8a3d", t, y: col(1) }, { label: "lateral", color: "#5eb1ff", t, y: col(2) }]} />
        <Chart title="Temperatura" unit="°C" cursorSync="env" series={[{ label: "ar", color: "#5eb1ff", t, y: col(3) }, { label: "pele (recuperação)", color: "#ff6b6b", t, y: col(4) }]} />
        <Chart title="Diferença de pressão baia − exterior" unit="Pa" cursorSync="env" series={[{ label: "ΔP", color: "#7ee787", t, y: col(7) }]} />
        <Chart title="Ruído da camada-limite" unit="dB" cursorSync="env" series={[{ label: "OASPL", color: "#d2a8ff", t, y: col(8) }]} />
      </div>
    </div>
  );
}
