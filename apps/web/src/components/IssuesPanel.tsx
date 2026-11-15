import type { Issue, Severity } from "@astra/core-js";

const icon: Record<Severity, string> = { error: "✕", warning: "!", info: "i" };
const title: Record<Severity, string> = { error: "Erros", warning: "Avisos", info: "Informações" };

export function IssuesPanel({ issues, busy, onPick }: { issues: Issue[]; busy: boolean; onPick: (partId: string) => void }) {
  const by = (s: Severity) => issues.filter((i) => i.severity === s);
  return (
    <section className="panel issues" aria-live="polite">
      <h2>
        Verificação física {busy && <small className="busy">calculando cargas…</small>}
      </h2>
      {issues.length === 0 && <p className="ok">Nenhum problema encontrado.</p>}
      {(["error", "warning", "info"] as Severity[]).map((s) =>
        by(s).length ? (
          <div key={s} className={`group ${s}`}>
            <h3>{title[s]} ({by(s).length})</h3>
            <ul>
              {by(s).map((i, k) => (
                <li key={`${i.code}-${k}`}>
                  <button onClick={() => i.parts[0] && onPick(i.parts[0])} disabled={!i.parts[0]}>
                    <span className="sev">{icon[s]}</span>
                    <span className="msg">
                      {i.message}
                      <em>{i.suggestion}</em>
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          </div>
        ) : null,
      )}
      <p className="note">
        O simulador nunca esconde um problema: você pode lançar assim mesmo e ver a consequência física.
      </p>
    </section>
  );
}
