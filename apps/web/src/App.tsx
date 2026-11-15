import { CoreClient, type CoreInfo } from "@astra/core-js";
import { useEffect, useState } from "react";

type CoreState =
  | { status: "loading" }
  | { status: "ready"; info: CoreInfo }
  | { status: "error"; message: string };

function useCore(): CoreState {
  const [state, setState] = useState<CoreState>({ status: "loading" });
  useEffect(() => {
    const worker = new Worker(new URL("./core.worker.ts", import.meta.url), { type: "module" });
    let alive = true;
    new CoreClient(worker).info().then(
      (info) => alive && setState({ status: "ready", info }),
      (e: Error) => alive && setState({ status: "error", message: e.message }),
    );
    return () => {
      alive = false;
      worker.terminate();
    };
  }, []);
  return state;
}

export function App() {
  const core = useCore();
  return (
    <main className="shell">
      <header>
        <h1>ASTRA</h1>
        <p>Simulador de foguetes · Fase 0</p>
      </header>
      <section className="panel" aria-live="polite">
        <h2>Núcleo de física</h2>
        {core.status === "loading" && <p>Carregando o núcleo WebAssembly…</p>}
        {core.status === "error" && <p className="bad">Falha ao carregar: {core.message}</p>}
        {core.status === "ready" && (
          <dl>
            <dt>Versão</dt>
            <dd>{core.info.version}</dd>
            <dt>Execução</dt>
            <dd>Web Worker + WebAssembly</dd>
            <dt>Autoteste de precisão</dt>
            <dd className={core.info.selfCheckError < 1e-8 ? "good" : "bad"}>
              erro {core.info.selfCheckError.toExponential(2)} m
            </dd>
          </dl>
        )}
      </section>
      <p className="note">
        Ferramenta de estudo e projeto. Não substitui análise de segurança, autorização de espaço
        aéreo nem as normas de lançamento locais.
      </p>
    </main>
  );
}
