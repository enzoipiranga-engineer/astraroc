import { CoreClient, buildBom, type Design, type ElecComponent, type ExampleFirmware, type Material, type PayloadPreset, type RuleSet, type SimOutput, type TemplateEntry, type Validation } from "@astra/core-js";
import { strFromU8, strToU8, unzipSync, zipSync } from "fflate";
import { tacviewFile } from "./tacview";
import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ElectronicsPanel } from "./components/ElectronicsPanel";
import { MissionReport } from "./components/MissionReport";
import { PayloadPanel } from "./components/PayloadPanel";
import { FinancePanel } from "./components/FinancePanel";
import { SpaceView } from "./components/SpaceView";
import { AnalysisPanel, flightReport } from "./components/AnalysisPanel";
import { IssuesPanel } from "./components/IssuesPanel";
import { JointPanel } from "./components/JointPanel";
import { LauncherPanel } from "./components/LauncherPanel";
import { OrbitalView } from "./components/OrbitalView";
import { PartPanel } from "./components/PartPanel";
import { Results } from "./components/Results";
import { RocketView } from "./components/RocketView";
import { RecoveryMap } from "./components/RecoveryMap";
import { SetupPanel } from "./components/SetupPanel";
import { SitePanel } from "./components/SitePanel";
import { WindPanel } from "./components/WindPanel";
import { download, downloadBytes, fileStem, toCsv, toKml } from "./export";
import { parseTrack, type TrackPoint } from "./geo";
import { commit, historyOf, loadSaved, redo, save, undo, type History } from "./state";
import { fmt, setUnitSystem, unitSystem, type UnitSystem } from "./units";
import { designFromHash, encodeDesign } from "./share";

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

function spawnWorker(): Worker {
  return new Worker(new URL("./core.worker.ts", import.meta.url), { type: "module" });
}

/** Cliente com worker próprio, para tarefas em paralelo. */
function spawnClient() {
  const w = spawnWorker();
  return { client: new CoreClient(w), stop: () => w.terminate() };
}

function useCore() {
  const client = useMemo(() => new CoreClient(spawnWorker()), []);
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
  const [mode, setMode] = useState<"rocket" | "orbital" | "space">("rocket");
  const [elecLib, setElecLib] = useState<ElecComponent[]>([]);
  const [example, setExample] = useState<ExampleFirmware | null>(null);
  const [units, setUnits] = useState<UnitSystem>(unitSystem());
  const [tour, setTour] = useState<number | null>(() => {
    try {
      return localStorage.getItem("astra.tour") ? null : 0;
    } catch {
      return null;
    }
  });
  const [payloadPresets, setPayloadPresets] = useState<PayloadPreset[]>([]);
  const [rulePresets, setRulePresets] = useState<[string, RuleSet][]>([]);
  const onTime = useCallback((t: number) => setSimTime(t), []);
  const pickAt = useCallback((t: number) => setSeek((s) => ({ t, n: (s?.n ?? 0) + 1 })), []);
  const seq = useRef(0);
  const prevRef = useRef<{ mass: number; cost: number; apogee: number } | null>(null);
  const [delta, setDelta] = useState<{ mass: number; cost: number; apogee: number } | null>(null);
  const elecLibRef = useRef<ElecComponent[]>([]);

  useEffect(() => {
    Promise.all([core.templates(), core.materials(), core.info(), core.electronics(), core.presets()]).then(
      ([t, m, info, el, pr]) => {
        setPayloadPresets(pr.payloads);
        setRulePresets(pr.rules);
        setElecLib(el.library);
        elecLibRef.current = el.library;
        setExample(el.firmware);
        setTemplates(t);
        setMaterials(m);
        setVersion(info.version);
        const shared = designFromHash(location.hash);
        if (shared) history.replaceState(null, "", location.pathname + location.search);
        const saved = shared ?? loadSaved();
        // Projetos salvos por versões anteriores podem não ter os campos novos.
        setHist(historyOf(saved ? { ...t[0]!.design, ...saved, zones: saved.zones ?? [], launcher: saved.launcher ?? [], electronics: saved.electronics ?? t[0]!.design.electronics, payloads: saved.payloads ?? [], launch: { ...t[0]!.design.launch, ...saved.launch, wind: saved.launch?.wind ?? { layers: [] } } } : t[0]!.design));
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
            // Efeito da última mudança: compara com a versão anterior verificada com voo.
            const cost = buildBom(design, elecLibRef.current, v.summary.partMasses ?? []).reduce((a, l) => a + l.totalBrl, 0);
            const now = { mass: v.summary.liftoffMass, cost, apogee: v.summary.apogee };
            const before = prevRef.current;
            setDelta(before ? { mass: now.mass - before.mass, cost: now.cost - before.cost, apogee: now.apogee - before.apogee } : null);
            prevRef.current = now;
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
    const a = Object.assign(document.createElement("a"), { href: url, download: `${fileStem(design.name)}.astra.json` });
    a.click();
    URL.revokeObjectURL(url);
  };
  const shareLink = async () => {
    if (!design) return;
    const url = `${location.origin}${location.pathname}#p=${encodeDesign(design)}`;
    if (url.length > 60_000) alert(`O link ficou com ${Math.round(url.length / 1000)} mil caracteres (firmware ou muitas peças); alguns aplicativos cortam links longos. Prefira Exportar.`);
    try {
      await navigator.clipboard.writeText(url);
      alert("Link copiado. Quem abrir verá este projeto (ele vai no próprio endereço, sem servidor).");
    } catch {
      prompt("Copie o link:", url);
    }
  };
  const toggleUnits = () => {
    const next: UnitSystem = units === "si" ? "imperial" : "si";
    setUnitSystem(next);
    setUnits(next);
  };
  const importOrk = async (f: File | undefined) => {
    if (!f) return;
    try {
      const bytes = new Uint8Array(await f.arrayBuffer());
      let xml: string;
      if (bytes[0] === 0x50 && bytes[1] === 0x4b) {
        // .ork compactado (zip): o XML é o primeiro arquivo .ork/.xml de dentro.
        const files = unzipSync(bytes);
        const name = Object.keys(files).find((k) => /\.(ork|xml)$/i.test(k)) ?? Object.keys(files)[0];
        if (!name) throw new Error("zip vazio");
        xml = strFromU8(files[name]!);
      } else xml = new TextDecoder().decode(bytes);
      const r = await core.analysis<{ design: Design; warnings: string[] }>("importOrk", { xml });
      change(r.design);
      setSelected(null);
      alert(`Importado do OpenRocket.\n\n${r.warnings.map((w) => `• ${w}`).join("\n")}`);
    } catch (e) {
      alert(`Não consegui importar: ${(e as Error).message}`);
    }
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
  const bom = s ? buildBom(design, elecLib, s.partMasses ?? []) : [];
  const bomTotal = bom.reduce((a, l) => a + l.totalBrl, 0);
  const bomMissing = bom.filter((l) => l.missing).length;
  const dlt = (v: number | undefined, unit: string, k = 1, digits = 0) =>
    delta && v !== undefined && Math.abs(v * k) >= 0.5 * 10 ** -digits ? <small className={v > 0 ? "up" : "down"}> {v > 0 ? "+" : "−"}{fmt(Math.abs(v * k), digits)} {unit}</small> : null;
  const nErr = issues.filter((i) => i.severity === "error").length;

  return (
    <div className="app">
      <header className="topbar">
        <h1>ASTRA</h1>
        <div className="modes" role="tablist">
          <button role="tab" aria-selected={mode === "rocket"} className={mode === "rocket" ? "on" : ""} onClick={() => setMode("rocket")}>Foguete</button>
          <button role="tab" aria-selected={mode === "orbital"} className={mode === "orbital" ? "on" : ""} onClick={() => setMode("orbital")}>Lançador orbital</button>
          <button role="tab" aria-selected={mode === "space"} className={mode === "space" ? "on" : ""} onClick={() => setMode("space")}>Espaço</button>
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
          <button onClick={() => void shareLink()} title="Copia um link com o projeto">Link</button>
          <button onClick={toggleUnits} title="Unidades dos campos">{units === "si" ? "SI" : "US"}</button>
          <label className="button" title="Projeto do OpenRocket">OpenRocket<input type="file" accept=".ork,.xml" hidden onChange={(e) => void importOrk(e.target.files?.[0])} /></label>
        </div>
        <button className="launch" onClick={() => void launch()} disabled={flying}>
          {flying ? "Simulando…" : nErr ? `LANÇAR (${nErr} ${nErr === 1 ? "erro" : "erros"})` : "LANÇAR"}
        </button>
          </>
        )}
      </header>

      {mode === "space" ? (
        <SpaceView core={core} />
      ) : mode === "orbital" ? (
        <OrbitalView core={core} ionToken={ionToken} />
      ) : (
        <>
      {s && (
        <div className="stats">
          <span>Massa <b>{fmt(s.liftoffMass * 1000, 0)} g</b>{!loadsBusy && dlt(delta?.mass, "g", 1000)}</span>
          <span>Margem <b className={s.staticMargin < 0 ? "bad" : s.staticMargin < 1 ? "warn" : ""}>{fmt(s.staticMargin, 2)} cal</b></span>
          <span>Empuxo/peso <b>{fmt(s.thrustToWeight, 1)}</b></span>
          <span>Apogeu previsto <b>{loadsBusy ? "…" : `${fmt(s.apogee, 0)} m`}</b>{!loadsBusy && dlt(delta?.apogee, "m")}</span>
          <span>Saída do trilho <b>{loadsBusy ? "…" : `${fmt(s.railExitSpeed, 1)} m/s`}</b></span>
          <span>Custo <b>R$ {fmt(bomTotal, 2)}</b>{dlt(delta?.cost, "R$", 1, 2)}{bomMissing > 0 && <small> ({bomMissing} sem preço)</small>}</span>
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
          <LauncherPanel design={design} core={core} onChange={change} />
          <SitePanel design={design} onChange={change} picking={picking} onTogglePick={() => setPicking((p) => !p)} ionToken={ionToken} onIonToken={saveIon} />
        </div>
        <div className="col">
          <IssuesPanel issues={issues} busy={loadsBusy} onPick={setSelected} />
          <WindPanel design={design} onChange={change} />
        </div>
      </div>

      <AnalysisPanel design={design} core={core} onChange={change} spawn={spawnClient} />
      <FinancePanel design={design} validation={validation} library={elecLib} materials={materials} core={core} onChange={change} />
      <PayloadPanel design={design} presets={payloadPresets} rulePresets={rulePresets} onChange={change} />
      <ElectronicsPanel design={design} validation={validation} core={core} library={elecLib} example={example} result={result} onChange={change} onFlight={setResult} />

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
          <button onClick={() => flightReport(design, validation, result)}>Relatório de voo (PDF)</button>
          {design.motor.type === "eng" && <button onClick={() => download(`${fileStem(design.name)}-motor.eng`, (design.motor as { text: string }).text, "text/plain")}>Motor (.eng)</button>}
        </div>
      )}
      {result && (
        <Results
          out={result}
          onPick={pickAt}
          onExport={(k) => {
            const stem = fileStem(design.name);
            if (k === "kml") download(`${stem}.kml`, toKml(design, result), "application/vnd.google-earth.kml+xml");
            else if (k === "kmz") downloadBytes(`${stem}.kmz`, zipSync({ "doc.kml": strToU8(toKml(design, result)) }), "application/vnd.google-earth.kmz");
            else if (k === "tacview") {
              const f = tacviewFile(result, design.name);
              downloadBytes(f.name, f.data, f.type);
            } else download(`${stem}.csv`, toCsv(result), "text/csv");
          }}
        />
      )}

      {result && <MissionReport payloads={result.payloads ?? []} competition={result.competition} />}
        </>
      )}

      {tour !== null && <Tour step={tour} onStep={setTour} />}
      <footer className="note">
        ASTRA {version} · ferramenta de estudo e projeto. Não substitui análise de segurança, autorização de espaço aéreo nem as normas de lançamento locais.
      </footer>
    </div>
  );
}

const TOUR = [
  { title: "Bem-vindo ao ASTRA", text: "Um simulador de foguetes com física real: cada peça tem material, massa e resistência, e as falhas são simuladas, não só avisadas." },
  { title: "1. Escolha um exemplo", text: "No topo, \"abrir exemplo\" carrega um foguete pronto: garrafa PET, competição de 500 m, com aviônica ou com carga útil." },
  { title: "2. Ajuste o projeto", text: "Clique numa peça no desenho ou na lista para mudar medidas e material. A verificação física à direita atualiza na hora, com sugestões de correção." },
  { title: "3. Lance", text: "LANÇAR simula o voo e abre o globo 3D, os gráficos e o mapa de recuperação. Os modos Lançador orbital e Espaço cobrem foguetes grandes, órbitas e pousos." },
  { title: "4. Vá além", text: "Mais abaixo: Monte Carlo, otimizador, finanças, carga útil e regras de competição, e eletrônica com Arduino emulado." },
];

function Tour({ step, onStep }: { step: number; onStep: (s: number | null) => void }) {
  const done = () => {
    try {
      localStorage.setItem("astra.tour", "1");
    } catch {
      /* sem armazenamento */
    }
    onStep(null);
  };
  const s = TOUR[step]!;
  return (
    <div className="tour" role="dialog" aria-label="Tutorial">
      <div className="tour-card">
        <h3>{s.title}</h3>
        <p>{s.text}</p>
        <div className="row-buttons">
          <button onClick={done}>Pular</button>
          {step > 0 && <button onClick={() => onStep(step - 1)}>Voltar</button>}
          {step < TOUR.length - 1 ? <button className="launch small" onClick={() => onStep(step + 1)}>Próximo</button> : <button className="launch small" onClick={done}>Começar</button>}
        </div>
        <small>{step + 1} / {TOUR.length}</small>
      </div>
    </div>
  );
}
