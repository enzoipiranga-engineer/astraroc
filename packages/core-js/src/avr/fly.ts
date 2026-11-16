/**
 * Voo com firmware no laço: o firmware roda no ATmega328P emulado, lê os sensores
 * virtuais alimentados pela física e, quando aciona um canal pirotécnico, a física é
 * refeita com o disparo naquele instante (as entradas antes do disparo não mudam, então
 * o firmware continua do mesmo ponto: uma única passada, sem rebobinar).
 */
import type { Design, ElecComponent, PyroEvent, SimOutput, Validation } from "../types";
import { SENSOR } from "../types";
import { Bmp280, Mpu6050, nmeaGga, type I2CDevice, type Reading } from "./devices";
import { parseHex } from "./hex";
import { Atmega328p, arduinoPin } from "./machine";
import { Rng } from "./rng";

export interface FlyOptions {
  /** Tempo ligado na rampa antes da ignição, s (o firmware calibra o solo nesse tempo). */
  padSeconds?: number;
  /** Limite de tempo emulado após a ignição, s. */
  maxSeconds?: number;
  /** Semente do ruído dos sensores. */
  seed?: number;
  /** Chamado de tempos em tempos com o progresso (0–1). */
  progress?: (f: number) => void;
}

export interface SerialLine {
  t: number;
  line: string;
}
export interface PinEvent {
  t: number;
  pin: string;
  high: boolean;
}
export interface PyroLog {
  ematch: string;
  pin: string;
  t: number;
  fired: boolean;
  reason: string;
}
/** Comparação sensor × verdade, a cada 0,05 s: t, p verdade, p medida, f_x verdade, f_x medida. */
export type TraceRow = [number, number, number, number, number];

export interface FlyResult {
  output: SimOutput;
  pyroEvents: PyroEvent[];
  serial: SerialLine[];
  pins: PinEvent[];
  pyro: PyroLog[];
  trace: TraceRow[];
  notes: string[];
  emulatedSeconds: number;
  resets: number;
}

const G0 = 9.80665;

/**
 * Erro da porta estática com o Mach: Δp = k(M)·q. Subsônico, uma porta lisa e longe de
 * degraus lê quase a estática (k ≈ −0,01). Na faixa transônica (0,85 < M < 1,15) a onda
 * de choque passa pela porta e a pressão salta (k ≈ +0,15): o altímetro "vê" uma queda
 * de altitude — a "Mach dip" conhecida dos altímetros comerciais (manuais PerfectFlite e
 * Altus Metrum recomendam travar a detecção de apogeu enquanto supersônico). Valores
 * ilustrativos, não medidos para uma geometria específica (ver docs/MODELOS.md).
 */
export function staticPortError(mach: number, q: number): number {
  const k = mach < 0.85 ? -0.01 : mach < 1.15 ? -0.01 + 0.16 * Math.sin((Math.PI * (mach - 0.85)) / 0.3) : 0.02;
  return k * q;
}

/** Interpolação linear nas linhas de verdade. */
function truthAt(rows: number[][], t: number): number[] {
  if (t <= rows[0]![0]!) return rows[0]!;
  const last = rows[rows.length - 1]!;
  if (t >= last[0]!) return last;
  let lo = 0;
  let hi = rows.length - 1;
  while (hi - lo > 1) {
    const mid = (lo + hi) >> 1;
    if (rows[mid]![0]! <= t) lo = mid;
    else hi = mid;
  }
  const a = rows[lo]!;
  const b = rows[hi]!;
  const f = (t - a[0]!) / (b[0]! - a[0]!);
  return a.map((x, i) => x + f * (b[i]! - x));
}

export function fly(
  design: Design,
  validation: Validation,
  library: ElecComponent[],
  simulate: (d: Design) => SimOutput,
  opts: FlyOptions = {},
): FlyResult {
  const e = design.electronics;
  const report = validation.electronics;
  if (!report) throw new Error("O projeto não tem eletrônica.");
  const fw = e.firmware;
  if (!fw) throw new Error("Nenhum firmware carregado.");
  const boardComp = e.components.find((c) => c.id === fw.board);
  const boardLib = boardComp && library.find((l) => l.id === boardComp.lib);
  if (!boardComp || !boardLib) throw new Error(`A placa "${fw.board}" do firmware não está na eletrônica.`);
  if (boardLib.mcu !== "atmega328p") throw new Error(`${boardLib.name} não tem emulação nesta versão (só ATmega328P: Uno, Nano, Pro Mini). Use hardware-in-the-loop.`);
  const program = parseHex(fw.hex);
  const clock = boardLib.clockHz ?? 16e6;
  const pad = opts.padSeconds ?? 3;
  const maxT = opts.maxSeconds ?? 300;
  const rng = new Rng(opts.seed ?? 1);
  const notes: string[] = [];
  const boardPowered = !report.issues.some((i) => i.severity === "error" && i.parts.includes(boardComp.part) && ["unpowered", "overvoltage", "undervoltage", "noGround", "noBattery"].includes(i.code));
  // Baia sem furo de respiro: o barômetro lê a pressão presa na baia (a do solo).
  const sealed = validation.issues.some((i) => i.code === "noStaticPort" && i.parts.includes(boardComp.part));
  if (sealed) notes.push("A placa está numa baia sem furo de respiro: o barômetro lê a pressão do solo o voo todo.");

  // Dispositivos no barramento.
  const devices: { dev: I2CDevice; comp: string; spec: ElecComponent }[] = [];
  for (const d of report.devices) {
    const spec = library.find((l) => l.id === d.lib);
    if (!spec || d.address === null) continue;
    if (!d.onBus) continue;
    if (!d.powered) {
      notes.push(`${spec.name} está sem alimentação correta: não responde no I2C.`);
      continue;
    }
    if (d.lib === "bmp280" || d.lib === "bmp280-modulo") devices.push({ dev: new Bmp280(d.address), comp: d.comp, spec });
    else if (d.lib === "mpu6050") devices.push({ dev: new Mpu6050(d.address), comp: d.comp, spec });
    else notes.push(`${spec.name} (0x${d.address.toString(16)}) não tem dispositivo virtual: não responde no I2C.`);
  }
  // GPS ligado no RX da placa.
  const rxPin = boardLib.pins.find((p) => p.kind === "rx")?.name;
  const gpsComp = e.components.find((c) => {
    const l = library.find((x) => x.id === c.lib);
    return l?.sensor?.type === "gps" && e.wires.some((w) => (w.a === `${c.id}:TX` && w.b === `${boardComp.id}:${rxPin}`) || (w.b === `${c.id}:TX` && w.a === `${boardComp.id}:${rxPin}`));
  });
  const gpsSpec = gpsComp ? library.find((l) => l.id === gpsComp.lib)?.sensor : undefined;
  const gpsBlocked = gpsComp ? report.devices.some((d) => d.comp === gpsComp.id && d.blocked) || report.issues.some((i) => i.code === "antennaBlocked" && i.parts.includes(gpsComp.part)) : false;

  // Fios mal crimpados.
  const loose = e.wires.filter((w) => (w.looseG ?? 0) > 0).map((w) => ({ w, broken: false }));

  const pyroPaths = report.pyro.filter((p) => p.board === boardComp.id && p.pin && arduinoPin(p.pin));
  for (const p of report.pyro) if (!p.pin) notes.push(`E-match ${p.ematch}: ${p.reason}.`);
  const pyroEvents: PyroEvent[] = [];
  const pyroLog: PyroLog[] = [];
  const serial: SerialLine[] = [];
  const pins: PinEvent[] = [];
  const trace: TraceRow[] = [];

  let current = { ...design, electronics: { ...e, pyroEvents: [] as PyroEvent[] } };
  let out = simulate(current);
  let rows = out.sensors ?? [];
  if (!rows.length) throw new Error("A simulação não devolveu a verdade dos sensores (a placa precisa estar numa peça do foguete).");
  // Na rampa: estado do solo, parado, com o acelerômetro lendo +1 g ao longo do eixo.
  const ground = rows[0]!.slice();
  for (const i of [SENSOR.airspeed, SENSOR.mach, SENSOR.q, SENSOR.fy, SENSOR.fz, SENSOR.wx, SENSOR.wy, SENSOR.wz, SENSOR.groundSpeed, SENSOR.vz]) ground[i] = 0;
  ground[SENSOR.fx] = G0;

  let boot = 0;
  let resets = 0;
  let halted = !boardPowered;
  if (halted) notes.push("A placa não liga: confira a alimentação nas verificações elétricas.");
  const make = () => {
    const m = new Atmega328p(program, clock, devices.map((d) => d.dev));
    m.onLine = (line) => serial.push({ t: boot + m.time - pad, line });
    return m;
  };
  let m = make();
  const pinState = new Map<string, boolean>();
  const watch = [...new Set([...pyroPaths.map((p) => p.pin!), "D13"])];
  const step = 1e-3;
  let tf = -pad;
  let lastFire = -Infinity;
  let nextGps = 0;
  let nextTrace = -pad;
  const truthEnd = () => rows[rows.length - 1]![0]!;
  // Viés fixo de cada sensor (sorteado uma vez).
  const bias = new Map(devices.map((d) => [d.comp, { p: d.spec.sensor?.type === "baro" ? d.spec.sensor.biasPa : 0, a: [rng.gauss() * 0.02 * G0, rng.gauss() * 0.02 * G0, rng.gauss() * 0.02 * G0] }]));

  for (;;) {
    const flightDone = tf > truthEnd() + 1;
    const allFired = pyroPaths.length > 0 && pyroPaths.every((p) => pyroLog.some((l) => l.ematch === p.ematch)) && tf > lastFire + 2;
    if (flightDone || allFired || tf > maxT) {
      if (tf > maxT) notes.push(`Emulação encerrada em ${maxT} s (limite).`);
      break;
    }
    tf += step;
    const tr = tf < 0 ? ground : truthAt(rows, tf);
    const gLoad = Math.hypot(tr[SENSOR.fx]!, tr[SENSOR.fy]!, tr[SENSOR.fz]!) / G0;
    for (const l of loose) {
      if (!l.broken && gLoad > (l.w.looseG ?? Infinity)) {
        l.broken = true;
        const ends = [l.w.a, l.w.b].map((x) => x.split(":")[0]);
        notes.push(`t = ${tf.toFixed(2)} s: o fio ${l.w.a} — ${l.w.b} soltou a ${gLoad.toFixed(1)} g.`);
        for (const d of devices) if (ends.includes(d.comp)) d.dev.offline = true;
        const powerPins = boardLib.pins.filter((p) => ["vin", "gnd"].includes(p.kind)).map((p) => `${boardComp.id}:${p.name}`);
        const batt = e.components.filter((c) => library.find((x) => x.id === c.lib)?.battery).map((c) => c.id);
        if (powerPins.includes(l.w.a) || powerPins.includes(l.w.b) || ends.some((x) => batt.includes(x!))) {
          halted = true;
          notes.push("A placa perdeu a alimentação em voo.");
        }
      }
    }
    for (const d of devices) {
      const s = d.spec.sensor;
      const b = bias.get(d.comp)!;
      const lag = d.dev instanceof Bmp280 ? d.dev.measureTime / 2 : 0.5e-3;
      const tl = tf - lag;
      const tt = tl < 0 ? ground : truthAt(rows, tl);
      let p = tt[SENSOR.pressure]! + (tl >= 0 ? staticPortError(tt[SENSOR.mach]!, tt[SENSOR.q]!) : 0);
      if (sealed) p = ground[SENSOR.pressure]!;
      if (s?.type === "baro") p = Math.min(s.rangePa[1], Math.max(s.rangePa[0], p + b.p + s.noisePa * rng.gauss()));
      const an = s?.type === "imu" ? s.accelNoiseG * G0 : 0;
      const wn = s?.type === "imu" ? (s.gyroNoiseDps * Math.PI) / 180 : 0;
      const r: Reading = {
        pressure: p,
        temperature: tt[SENSOR.temperature]!,
        accel: [0, 1, 2].map((i) => tt[SENSOR.fx + i]! + b.a[i]! + an * rng.gauss()) as [number, number, number],
        gyro: [0, 1, 2].map((i) => tt[SENSOR.wx + i]! + wn * rng.gauss()) as [number, number, number],
      };
      (d.dev as Bmp280 | Mpu6050).reading = r;
      if (tf >= nextTrace && d.dev instanceof Bmp280) {
        trace.push([tf, tr[SENSOR.pressure]!, p, tr[SENSOR.fx]!, r.accel[0]]);
      }
    }
    if (tf >= nextTrace) nextTrace += 0.05;
    // GPS: NMEA GGA a 5 Hz; perde a solução acima de 4 g, pelos limites COCOM ou com a antena bloqueada.
    if (gpsSpec?.type === "gps" && tf >= nextGps) {
      nextGps = tf + 1 / gpsSpec.rateHz;
      const fix = !gpsBlocked && gLoad < gpsSpec.maxG && !(tr[SENSOR.groundSpeed]! > gpsSpec.cocomSpeed && tr[SENSOR.alt]! > gpsSpec.cocomAlt) && tf > -pad + 1;
      const err = gpsSpec.accuracyM / 111_320;
      if (!halted) m.feedSerial(nmeaGga(43200 + tf, tr[SENSOR.lat]! + err * rng.gauss(), tr[SENSOR.lon]! + err * rng.gauss(), tr[SENSOR.alt]! + 1.5 * gpsSpec.accuracyM * rng.gauss(), fix, fix ? 8 : 0));
    }
    if (halted) continue;
    m.runUntil(tf + pad - boot);
    m.pumpRx();
    for (const pin of watch) {
      const hi = m.pinHigh(pin);
      if (hi === (pinState.get(pin) ?? false)) continue;
      pinState.set(pin, hi);
      pins.push({ t: tf, pin, high: hi });
      if (!hi) continue;
      for (const path of pyroPaths.filter((p) => p.pin === pin)) {
        if (pyroLog.some((l) => l.ematch === path.ematch)) continue;
        const t = Math.max(tf, 0);
        if (tf < 0) notes.push(`O firmware acionou ${pin} na rampa (t = ${tf.toFixed(2)} s): o e-match ${path.ematch} dispararia antes da decolagem.`);
        pyroLog.push({ ematch: path.ematch, pin, t, fired: path.fires, reason: path.reason });
        if (!path.fires) {
          notes.push(`t = ${tf.toFixed(2)} s: ${pin} acionado, mas o e-match ${path.ematch} não acende (${path.reason}).`);
          continue;
        }
        lastFire = tf;
        pyroEvents.push({ ematch: path.ematch, t });
        current = { ...current, electronics: { ...current.electronics, pyroEvents: [...pyroEvents] } };
        out = simulate(current);
        rows = out.sensors ?? rows;
        if (path.brownout) {
          resets++;
          notes.push(`t = ${tf.toFixed(2)} s: a tensão caiu no disparo e a placa reiniciou (brownout).`);
          boot = tf + pad;
          m = make();
          pinState.clear();
        }
      }
    }
    if (opts.progress && Math.round(tf * 1000) % 500 === 0) opts.progress(Math.min(1, (tf + pad) / (truthEnd() + pad + 1)));
  }
  if (pyroPaths.length && !pyroEvents.length) notes.push("O firmware não acionou nenhum canal pirotécnico durante o voo.");
  return { output: out, pyroEvents, serial, pins, pyro: pyroLog, trace, notes, emulatedSeconds: tf + pad, resets };
}
