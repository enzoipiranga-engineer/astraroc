import { useMemo, useState } from "react";
import type { CoreClient, Design, EComp, ElecComponent, ExampleFirmware, FlyResult, Part, SimOutput, Validation, Wire } from "@astra/core-js";
import { SENSOR } from "@astra/core-js";
import { Chart } from "./Chart";
import { NumberField, SelectField } from "./Field";

interface Props {
  design: Design;
  validation: Validation | null;
  core: CoreClient;
  library: ElecComponent[];
  example: ExampleFirmware | null;
  result: SimOutput | null;
  onChange: (d: Design) => void;
  onFlight: (out: SimOutput) => void;
}

const CATEGORY: Record<string, string> = {
  placa: "Placas",
  sensor: "Sensores",
  radio: "Rádio",
  armazenamento: "Armazenamento",
  atuador: "Atuadores",
  energia: "Baterias",
  pirotecnia: "Pirotecnia",
  misc: "Outros",
};

const download = (name: string, text: string, type = "text/plain") => {
  const url = URL.createObjectURL(new Blob([text], { type }));
  Object.assign(document.createElement("a"), { href: url, download: name }).click();
  URL.revokeObjectURL(url);
};

function uniqueId(base: string, taken: Set<string>): string {
  let i = 1;
  while (taken.has(`${base}${i}`)) i++;
  return `${base}${i}`;
}

/** Peças estruturais em que um componente pode ser fixado. */
const mounts = (d: Design) => d.parts.filter((p) => ["bulkhead", "coupler", "bodyTube", "noseCone", "centeringRing"].includes(p.kind.type));

export function ElectronicsPanel({ design, validation, core, library, example, result, onChange, onFlight }: Props) {
  const e = design.electronics;
  const [libId, setLibId] = useState("arduino-nano");
  const [where, setWhere] = useState("__new");
  const [mount, setMount] = useState(() => mounts(design)[0]?.id ?? "");
  const [a, setA] = useState("");
  const [b, setB] = useState("");
  const [fly, setFly] = useState<FlyResult | null>(null);
  const [busy, setBusy] = useState(false);
  const report = validation?.electronics;
  const lib = (c: EComp) => library.find((l) => l.id === c.lib);
  const setE = (next: Partial<Design["electronics"]>) => onChange({ ...design, electronics: { ...e, ...next } });
  const massParts = design.parts.filter((p) => p.kind.type === "mass");

  const add = () => {
    const spec = library.find((l) => l.id === libId);
    if (!spec) return;
    const ids = new Set([...e.components.map((c) => c.id), ...design.parts.map((p) => p.id)]);
    const id = uniqueId(spec.id.split("-")[0]!, ids);
    let parts = design.parts;
    let joints = design.joints;
    let part = where;
    if (where === "__new") {
      const at = design.parts.find((p) => p.id === mount);
      if (!at) return alert("Escolha uma peça onde fixar o componente.");
      part = `${id}-peca`;
      const p: Part = {
        id: part,
        name: spec.name,
        material: "g10-fr4",
        position: at.position,
        kind: { type: "mass", mass: spec.massG / 1000, length: Math.max(spec.lengthMm, 1) / 1000, diameter: Math.max(spec.widthMm, 1) / 1000, needsStaticPort: spec.sensor?.type === "baro" },
      };
      parts = [...parts, p];
      joints = [...joints, { id: `j-${part}`, a: part, b: at.id, kind: { type: "screws", count: 2, diameter: 0.003, material: "aco-parafuso-8.8" } }];
    }
    const comp: EComp = { id, lib: spec.id, part };
    const firmware = e.firmware ?? (spec.category === "placa" && example && spec.mcu === "atmega328p" ? { board: id, name: example.name, hex: example.hex } : null);
    onChange({ ...design, parts, joints, electronics: { ...e, components: [...e.components, comp], firmware } });
  };

  const remove = (c: EComp) => {
    const components = e.components.filter((x) => x.id !== c.id);
    const wires = e.wires.filter((w) => !w.a.startsWith(`${c.id}:`) && !w.b.startsWith(`${c.id}:`));
    const orphan = c.part.endsWith("-peca") && !components.some((x) => x.part === c.part);
    const parts = orphan ? design.parts.filter((p) => p.id !== c.part) : design.parts;
    const joints = orphan ? design.joints.filter((j) => j.a !== c.part && j.b !== c.part) : design.joints;
    const firmware = e.firmware?.board === c.id ? null : e.firmware;
    onChange({ ...design, parts, joints, electronics: { ...e, components, wires, firmware: firmware ?? null } });
  };

  const pinOptions = useMemo(
    () => [
      { value: "", label: "escolha um pino…" },
      ...e.components.flatMap((c) => (lib(c)?.pins ?? []).map((p) => ({ value: `${c.id}:${p.name}`, label: `${c.id}:${p.name} (${p.kind})`, group: `${c.id} — ${lib(c)?.name ?? c.lib}` }))),
    ],
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [e.components, library],
  );
  const addWire = () => {
    if (!a || !b || a === b) return;
    setE({ wires: [...e.wires, { a, b }] });
    setA("");
    setB("");
  };
  const setWire = (i: number, w: Wire) => setE({ wires: e.wires.map((x, k) => (k === i ? w : x)) });

  const boards = e.components.filter((c) => lib(c)?.category === "placa");
  const loadHex = async (f: File | undefined) => {
    if (!f) return;
    const board = e.firmware?.board ?? boards[0]?.id;
    if (!board) return alert("Adicione uma placa antes de carregar o firmware.");
    setE({ firmware: { board, name: f.name, hex: await f.text() } });
  };

  const runFirmware = async () => {
    setBusy(true);
    try {
      const r = await core.fly(design);
      setFly(r);
      onFlight(r.output);
    } catch (err) {
      alert(`Falha no voo com firmware: ${(err as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="panel electronics">
      <h2>
        Eletrônica e firmware <small>Arduino emulado no voo</small>
      </h2>
      <div className="elec-grid">
        <div>
          <h3>Componentes</h3>
          {e.components.length === 0 && <p className="hint">Sem eletrônica. Adicione uma placa, sensores, bateria, MOSFET e e-match — ou abra o exemplo "Competição 500 m com aviônica".</p>}
          <ul className="elec-list">
            {e.components.map((c) => {
              const l = lib(c);
              const dev = report?.devices.find((d) => d.comp === c.id);
              return (
                <li key={c.id}>
                  <b>{c.id}</b> {l?.name ?? c.lib} <small>em {design.parts.find((p) => p.id === c.part)?.name ?? c.part}</small>
                  {dev?.address != null && <small className={dev.onBus ? "ok" : "bad"}> · I2C 0x{dev.address.toString(16)}{dev.onBus ? "" : " fora do barramento"}</small>}
                  {l?.battery && (
                    <NumberField label="Carga" unit="ratio" value={c.charge ?? 1} onChange={(v) => v !== null && setE({ components: e.components.map((x) => (x.id === c.id ? { ...x, charge: v } : x)) })} />
                  )}
                  {l && l.i2c.length > 1 && (
                    <SelectField
                      label="Endereço I2C"
                      value={String(c.i2cAddress ?? l.i2c[0])}
                      options={l.i2c.map((x) => ({ value: String(x), label: `0x${x.toString(16)}` }))}
                      onChange={(v) => setE({ components: e.components.map((x) => (x.id === c.id ? { ...x, i2cAddress: Number(v) } : x)) })}
                    />
                  )}
                  <button className="link danger" onClick={() => remove(c)}>remover</button>
                </li>
              );
            })}
          </ul>
          <div className="row">
            <SelectField label="Adicionar" value={libId} options={library.map((l) => ({ value: l.id, label: l.name, group: CATEGORY[l.category] ?? l.category }))} onChange={setLibId} />
            <SelectField label="Montar em" value={where} options={[{ value: "__new", label: "nova peça…" }, ...massParts.map((p) => ({ value: p.id, label: p.name }))]} onChange={setWhere} />
          </div>
          {where === "__new" && <SelectField label="Fixada (2 parafusos M3) em" value={mount} options={mounts(design).map((p) => ({ value: p.id, label: p.name }))} onChange={setMount} />}
          <button onClick={add}>Adicionar componente</button>
          {library.find((l) => l.id === libId)?.notes && <p className="hint">{library.find((l) => l.id === libId)!.notes}</p>}
        </div>

        <div>
          <h3>Fiação</h3>
          <ul className="elec-list wires">
            {e.wires.map((w, i) => (
              <li key={`${w.a}-${w.b}-${i}`}>
                <span className="mono">{w.a} — {w.b}</span>
                <NumberField label="Solta acima de" unit="count" optional value={w.looseG ?? null} help="Fio mal crimpado: solta quando a aceleração passa deste valor (g)." onChange={(v) => setWire(i, { ...w, looseG: v })} />
                <button className="link danger" onClick={() => setE({ wires: e.wires.filter((_, k) => k !== i) })}>×</button>
              </li>
            ))}
          </ul>
          <div className="row">
            <SelectField label="De" value={a} options={pinOptions} onChange={setA} />
            <SelectField label="Para" value={b} options={pinOptions} onChange={setB} />
          </div>
          <button onClick={addWire} disabled={!a || !b || a === b}>Ligar fio</button>
        </div>

        <div>
          <h3>Firmware</h3>
          {e.firmware ? (
            <p className="hint mono">{e.firmware.name} → {e.firmware.board} ({Math.round(e.firmware.hex.length / 1024)} kB de HEX)</p>
          ) : (
            <p className="hint">Nenhum firmware carregado.</p>
          )}
          {boards.length > 1 && (
            <SelectField label="Placa" value={e.firmware?.board ?? boards[0]!.id} options={boards.map((c) => ({ value: c.id, label: `${c.id} (${lib(c)?.name})` }))} onChange={(v) => e.firmware && setE({ firmware: { ...e.firmware, board: v } })} />
          )}
          <div className="row-buttons">
            <label className="button">
              Carregar .hex
              <input type="file" accept=".hex,.ihex" hidden onChange={(ev) => void loadHex(ev.target.files?.[0])} />
            </label>
            {example && boards[0] && <button onClick={() => setE({ firmware: { board: e.firmware?.board ?? boards[0]!.id, name: example.name, hex: example.hex } })}>Usar exemplo</button>}
            {example && <button onClick={() => download("apogeu.c", example.source)}>Código C do exemplo</button>}
          </div>
          <p className="hint">
            Compile o seu código para ATmega328P (Arduino IDE: Sketch → Exportar binário compilado; ou <span className="mono">avr-gcc -mmcu=atmega328p</span>) e carregue o .hex. Emulados: BMP280 e MPU-6050 no I2C, GPS NMEA na serial, pinos digitais.
          </p>
          <button className="launch small" onClick={() => void runFirmware()} disabled={busy || !e.firmware}>
            {busy ? "Emulando o firmware…" : "VOAR COM FIRMWARE"}
          </button>
          <Hil result={result} />
        </div>
      </div>

      {report && (
        <div className="elec-report">
          <h3>Análise elétrica</h3>
          <p>
            Consumo em espera <b>{report.standbyMa.toFixed(0)} mA</b> · autonomia <b>{report.autonomyH === null ? "—" : `${report.autonomyH.toFixed(1)} h`}</b> (espera prevista{" "}
            <input className="tiny" type="number" min={0} step={0.5} value={e.standbyHours} onChange={(ev) => setE({ standbyHours: Number(ev.target.value) || 0 })} /> h)
            {report.radioRangeKm !== null && <> · alcance de rádio estimado <b>{report.radioRangeKm.toFixed(1)} km</b></>}
          </p>
          {report.pyro.map((p) => (
            <p key={p.ematch} className={`pcheck ${p.fires ? "ok" : "bad"}`}>
              E-match <b>{p.ematch}</b>
              {p.pin && <> pelo pino {p.pin}</>}: {p.fires ? "acende" : "NÃO acende"} — {p.reason}
              {p.brownout && " · a placa reinicia no disparo (brownout)"}
            </p>
          ))}
        </div>
      )}

      {fly && <FirmwareResults fly={fly} />}
    </section>
  );
}

function FirmwareResults({ fly }: { fly: FlyResult }) {
  const t = fly.trace.map((r) => r[0]);
  return (
    <div className="fw-results">
      <h3>
        Resultado do firmware <small>{fly.emulatedSeconds.toFixed(1)} s emulados{fly.resets ? ` · ${fly.resets} reinício(s)` : ""}</small>
      </h3>
      {fly.pyro.map((p) => (
        <p key={p.ematch} className={`pcheck ${p.fired ? "ok" : "bad"}`}>
          t = {p.t.toFixed(2)} s: {p.pin} acionou o e-match {p.ematch} {p.fired ? "(disparou)" : `(não acendeu: ${p.reason})`}
        </p>
      ))}
      {fly.notes.map((n) => (
        <p key={n} className="pcheck bad">{n}</p>
      ))}
      <div className="fw-grid">
        <pre className="console" aria-label="Serial">
          {fly.serial.slice(-400).map((s) => `${s.t.toFixed(3).padStart(8)}  ${s.line}`).join("\n")}
        </pre>
        <div>
          {fly.trace.length > 0 && (
            <Chart
              title="Pressão: verdade × o que o BMP280 entregou"
              unit="Pa"
              cursorSync="fw"
              series={[
                { label: "verdade", color: "#5eb1ff", t, y: fly.trace.map((r) => r[1]) },
                { label: "medida", color: "#ff8a3d", t, y: fly.trace.map((r) => r[2]) },
              ]}
            />
          )}
          <ul className="elec-list">
            {fly.pins.slice(0, 40).map((p, i) => (
              <li key={i} className="mono">t = {p.t.toFixed(3)} s · {p.pin} {p.high ? "ALTO" : "baixo"}</li>
            ))}
          </ul>
        </div>
      </div>
    </div>
  );
}

/** Interface mínima da Web Serial API (Chrome/Edge). */
interface SerialPortLike {
  open(o: { baudRate: number }): Promise<void>;
  close(): Promise<void>;
  readable: ReadableStream<Uint8Array> | null;
  writable: WritableStream<Uint8Array> | null;
}

/**
 * Hardware-in-the-loop pela Web Serial: envia a verdade dos sensores do último voo à placa
 * real, em tempo real, e mostra o que ela responde. Protocolo em docs/HIL.md.
 */
function Hil({ result }: { result: SimOutput | null }) {
  const serial = (navigator as Navigator & { serial?: { requestPort(): Promise<SerialPortLike> } }).serial;
  const [log, setLog] = useState<string[]>([]);
  const [running, setRunning] = useState(false);
  if (!serial) return <p className="hint">Hardware-in-the-loop: use Chrome ou Edge (Web Serial) para ligar a placa real por USB.</p>;
  const rows = result?.sensors ?? [];
  const start = async () => {
    if (!rows.length) return alert("Rode um voo primeiro: o HIL reenvia a verdade dos sensores desse voo.");
    const port = await serial.requestPort();
    await port.open({ baudRate: 115200 });
    setRunning(true);
    const push = (l: string) => setLog((x) => [...x.slice(-200), l]);
    const reader = port.readable!.pipeThrough(new TextDecoderStream() as unknown as ReadableWritablePair<string, Uint8Array>).getReader();
    let buf = "";
    void (async () => {
      for (;;) {
        const { value, done } = await reader.read();
        if (done) break;
        buf += value;
        let k;
        while ((k = buf.indexOf("\n")) >= 0) {
          push(buf.slice(0, k).trim());
          buf = buf.slice(k + 1);
        }
      }
    })();
    const writer = port.writable!.getWriter();
    const enc = new TextEncoder();
    const t0 = performance.now();
    for (let i = 0; i < rows.length; i += 2) {
      const r = rows[i]!;
      const wait = r[SENSOR.t]! * 1000 - (performance.now() - t0);
      if (wait > 0) await new Promise((res) => setTimeout(res, wait));
      const f = (x: number) => x.toFixed(3);
      await writer.write(enc.encode(`S,${f(r[SENSOR.t]!)},${f(r[SENSOR.pressure]!)},${f(r[SENSOR.temperature]!)},${f(r[SENSOR.fx]!)},${f(r[SENSOR.fy]!)},${f(r[SENSOR.fz]!)},${f(r[SENSOR.wx]!)},${f(r[SENSOR.wy]!)},${f(r[SENSOR.wz]!)}\n`));
    }
    await writer.write(enc.encode("E\n"));
    writer.releaseLock();
    await reader.cancel().catch(() => undefined);
    await port.close().catch(() => undefined);
    setRunning(false);
  };
  return (
    <div className="hil">
      <button onClick={() => void start().catch((err: Error) => { setRunning(false); alert(err.message); })} disabled={running}>
        {running ? "HIL rodando…" : "Hardware-in-the-loop (USB)"}
      </button>
      <p className="hint">Envia a verdade do último voo à placa real (115200 baud, protocolo em docs/HIL.md). Não testado com hardware nesta versão.</p>
      {log.length > 0 && <pre className="console small">{log.join("\n")}</pre>}
    </div>
  );
}
