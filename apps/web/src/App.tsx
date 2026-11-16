import { CoreClient, type Design, type Material, type SimOutput, type TemplateEntry, type Validation } from "@astra/core-js";
import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { IssuesPanel } from "./components/IssuesPanel";
import { JointPanel } from "./components/JointPanel";
import { OrbitalView } from "./components/OrbitalView";
import { PartPanel } from "./components/PartPanel";
import { Results } from "./components/Results";
import { RocketView } from "./components/RocketView";
import { RecoveryMap } from "./components/RecoveryMap";
import { SetupPanel } from "./components/SetupPanel";
import { SitePanel } from "./components/SitePanel";
import { WindPanel } from "./components/WindPanel";
import { download, toCsv, toKml } from "./export";
import { parseTrack, type TrackPoint } from "./geo";
import { commit, historyOf, loadSaved, redo, save, undo, type History } from "./state";
import { fmt } from "./units";

const GlobeView = lazy(() => import("./components/GlobeView").then((m) => ({ default: m.GlobeView })));

const ION_KEY = "astra.ionToken";
const readIon = () => {
  try {
    return localStorage.getItem(ION_KEY) ?? "";
  } catch {
    return "";
  }
};

type Boot = { status: "loading" } | { status: "error"; message: string } | { status: "ready" };

function useCore() {
  const client = useMemo(() => new CoreClient(new Worker(new URL("./core.worker.ts", import.meta.url), { type: "module" })), []);
  return client;
}

export function App() {
  const core = useCore();
  const [boot, setBoot] = useState<Boot>({ status: "loading" });
  const [templates, setTemplates] = useState<TemplateEntry[]>([]);
  const [materials, setMaterials] = useState<Material[]>([]);
  const [hist, setHist] = useState<History | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [validation, setValidation] = useState<Validation | null>(null);
  const [loadsBusy, setLoadsBusy] = useState(false);
  const [result, setResult] = useState<SimOutput | null>(null);
  const [flying, setFlying] = useState(false);
  const [version, setVersion] = useState("");
  const [ionToken, setIonToken] = useState(readIon);
  const [simTime, setSimTime] = useState(0);
  const [seek, setSeek] = useState<{ t: number; n: number } | null>(null);
  const [picking, setPicking] = useState(false);
  const [gps, setGps] = useState<TrackPoint[]>([]);
  const [mode, setMode] = useState<"rocket" | "orbital">("rocket");
  const onTime = useCallback((t: number) => setSimTime(t), []);
  const pickAt = useCallback((t: number) => setSeek((s) => ({ t, n: (s?.n ?? 0) + 1 })), []);
  const seq = useRef(0);

  useEffect(() => {
    Promise.all([core.templates(), core.materials(), core.info()]).then(
      ([t, m, info]) => {
        setTemplates(t);
        setMaterials(m);
        setVersion(info.version);
        setHist(historyOf(loadSaved() ?? t[0]!.design));
        setBoot({ status: "ready" });
      },
      (e: Error) => setBoot({ status: "error", message: e.message }),
    );
  }, [core]);

  const design = hist?.present ?? null;

  // Verificação contínua: rápida a cada mudança, com cargas de voo logo depois.
  useEffect(() => {
    if (!design) return;
    save(design);
    const id = ++seq.current;
    core.validate(design, false).then((v) => id === seq.current && setValidation(v), () => undefined);
    setLoadsBusy(true);
    const timer = setTimeout(() => {
      core.validate(design, true).then(
        (v) => {
          if (id === seq.current) {
            setValidation(v);
            setLoadsBusy(false);
          }
        },
        () => id === seq.current && setLoadsBusy(false),
      );
    }, 500);
    return () => clearTimeout(timer);
  }, [core, design]);

  const change = useCallback((d: Design) => {
    setHist((h) => (h ? commit(h, d) : h));
    setResult(null);
  }, []);
  const pickLocation = useCallback(
    (lat: number, lon: number) => {
      setHist((h) => (h ? commit(h, { ...h.present, launch: { ...h.present.launch, latDeg: +lat.toFixed(6), lonDeg: +lon.toFixed(6) } }) : h));
      setPicking(false);
    },
    [],
  );
  const saveIon = (t: string) => {
    try {
      if (t) localStorage.setItem(ION_KEY, t);
      else localStorage.removeItem(ION_KEY);
    } catch {
      /* sem armazenamento: vale só nesta sessão */
    }
    setIonToken(t);
  };
  const importGps = async (f: File | undefined) => {
    if (!f) return;
    const pts = parseTrack(f.name, await f.text());
    if (pts.length < 2) alert("Não encontrei pontos de GPS no arquivo (GPX, KML ou CSV lat,lon,alt).");
    setGps(pts);
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey) || (e.target as HTMLElement).tagName === "INPUT") return;
      if (e.key === "z" && !e.shiftKey) setHist((h) => h && undo(h));
      if (e.key === "y" || (e.key === "z" && e.shiftKey)) setHist((h) => h && redo(h));
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const launch = async () => {
    if (!design) return;
    setFlying(true);
    try {
      setResult(await core.simulate(design));
    } catch (e) {
      alert(`Falha na simulação: ${(e as Error).message}`);
    } finally {
      setFlying(false);
    }
  };

  const exportJson = () => {
    if (!design) return;
    const url = URL.createObjectURL(new Blob([JSON.stringify(design, null, 2)], { type: "application/json" }));
    const a = Object.assign(document.createElement("a"), { href: url, download: `${design.name.replace(/[^\w-]+/g, "_")}.astra.json` });
    a.click();
    URL.revokeObjectURL(url);
  };
  const importJson = async (f: File | undefined) => {
    if (!f) return;
    try {
      change(JSON.parse(await f.text()) as Design);
    } catch {
      alert("Arquivo de projeto inválido.");
    }
  };

  if (boot.status === "loading") return <main className="boot">Carregando o núcleo de física…</main>;
  if (boot.status === "error" || !design || !hist) return <main className="boot bad">Falha ao carregar o núcleo: {boot.status === "error" ? boot.message : ""}</main>;

  const issues = validation?.issues ?? [];
  const errorParts = new Set(issues.filter((i) => i.severity === "error").flatMap((i) => i.parts));
  const errorJoints = new Set(issues.filter((i) => i.severity !== "info").flatMap((i) => i.joints));
  const s = validation?.summary;
  const nErr = issues.filter((i) => i.severity === "error").length;

  return (
    <div className="app">
      <header className="topbar">
        <h1>ASTRA</h1>
        <div className="modes" role="tablist">
          <button role="tab" aria-selected={mode === "rocket"} className={mode === "rocket" ? "on" : ""} onClick={() => setMode("rocket")}>Foguete</button>
          <button role="tab" aria-selected={mode === "orbital"} className={mode === "orbital" ? "on" : ""} onClick={() => setMode("orbital")}>Lançador orbital</button>
        </div>
        {mode === "rocket" && (
          <>
        <label className="tpl">
          <span>Exemplo</span>
          <select value="" onChange={(e) => { const t = templates.find((x) => x.id === e.target.value); if (t) { change(structuredClone(t.design)); setSelected(null); } }}>
            <option value="">abrir exemplo…</option>
            {templates.map((t) => <option key={t.id} value={t.id}>{t.design.name}</option>)}
          </select>
        </label>
        <input className="name" value={design.name} onChange={(e) => change({ ...design, name: e.target.value })} aria-label="Nome do projeto" />
        <div className="tools">
          <button onClick={() => setHist(undo(hist))} disabled={!hist.past.length} title="Ctrl+Z">Desfazer</button>
          <button onClick={() => setHist(redo(hist))} disabled={!hist.future.length} title="Ctrl+Y">Refazer</button>
          <button onClick={exportJson}>Exportar</button>
          <label className="button">Importar<input type="file" accept=".json" hidden onChange={(e) => void importJson(e.target.files?.[0])} /></label>
        </div>
        <button className="launch" onClick={() => void launch()} disabled={flying}>
          {flying ? "Simulando…" : nErr ? `LANÇAR (${nErr} ${nErr === 1 ? "erro" : "erros"})` : "LANÇAR"}
        </button>
          </>
        )}
      </header>

      {mode === "orbital" ? (
        <OrbitalView core={core} ionToken={ionToken} />
      ) : (
        <>
      {s && (
        <div className="stats">
          <span>Massa <b>{fmt(s.liftoffMass * 1000, 0)} g</b></span>
          <span>Margem <b className={s.staticMargin < 0 ? "bad" : s.staticMargin < 1 ? "warn" : ""}>{fmt(s.staticMargin, 2)} cal</b></span>
          <span>Empuxo/peso <b>{fmt(s.thrustToWeight, 1)}</b></span>
          <span>Apogeu previsto <b>{loadsBusy ? "…" : `${fmt(s.apogee, 0)} m`}</b></span>
          <span>Saída do trilho <b>{loadsBusy ? "…" : `${fmt(s.railExitSpeed, 1)} m/s`}</b></span>
          <span>Custo <b>R$ {fmt(s.cost, 2)}</b>{s.partsWithoutCost > 0 && <small> ({s.partsWithoutCost} sem preço)</small>}</span>
        </div>
      )}

      <RocketView design={design} validation={validation} selected={selected} errorParts={errorParts} onSelect={setSelected} />

      <div className="columns">
        <div className="col">
          <PartPanel design={design} materials={materials} selected={selected} errorParts={errorParts} onSelect={setSelected} onChange={change} />
        </div>
        <div className="col">
          <JointPanel design={design} materials={materials} errorJoints={errorJoints} selectedPart={selected} onChange={change} />
          <SetupPanel design={design} onChange={change} />
          <SitePanel design={design} onChange={change} picking={picking} onTogglePick={() => setPicking((p) => !p)} ionToken={ionToken} onIonToken={saveIon} />
        </div>
        <div className="col">
          <IssuesPanel issues={issues} busy={loadsBusy} onPick={setSelected} />
          <WindPanel design={design} onChange={change} />
        </div>
      </div>

      {(result || picking) && (
        <div className="flight-view">
          <Suspense fallback={<section className="panel globe">Carregando o globo 3D…</section>}>
            <GlobeView
              design={design}
              validation={validation}
              out={result ?? { bodies: [], events: [], warnings: [] }}
              gps={gps}
              ionToken={ionToken}
              seek={seek}
              onTime={onTime}
              pickMode={picking}
              onPickLocation={pickLocation}
            />
          </Suspense>
          {result && <RecoveryMap design={design} out={result} t={simTime} />}
        </div>
      )}
      {result && (
        <div className="gps-import">
          <label className="button">
            Sobrepor GPS real (GPX, KML, CSV)
            <input type="file" accept=".gpx,.kml,.csv,.txt" hidden onChange={(e) => void importGps(e.target.files?.[0])} />
          </label>
          {gps.length > 0 && <span className="hint">{gps.length} pontos de GPS sobrepostos em amarelo.</span>}
        </div>
      )}
      {result && (
        <Results
          out={result}
          onPick={pickAt}
          onExport={(k) =>
            k === "kml"
              ? download(`${design.name.replace(/[^\w-]+/g, "_")}.kml`, toKml(design, result), "application/vnd.google-earth.kml+xml")
              : download(`${design.name.replace(/[^\w-]+/g, "_")}.csv`, toCsv(result), "text/csv")
          }
        />
      )}

        </>
      )}

      <footer className="note">
        ASTRA {version} · ferramenta de estudo e projeto. Não substitui análise de segurança, autorização de espaço aéreo nem as normas de lançamento locais.
      </footer>
    </div>
  );
}
