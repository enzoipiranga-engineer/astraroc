/**
 * Dispositivos I2C virtuais que respondem como os chips reais, a partir das grandezas
 * "medidas" (verdade + ruído + viés + atraso, calculadas pelo chamador).
 */

/** O que um sensor enxerga num instante. */
export interface Reading {
  /** Pressão na porta estática, Pa. */
  pressure: number;
  /** Temperatura, K. */
  temperature: number;
  /** Força específica no corpo, m/s². */
  accel: [number, number, number];
  /** Velocidade angular no corpo, rad/s. */
  gyro: [number, number, number];
}

export interface I2CDevice {
  readonly address: number;
  /** Desconectado (fio solto, sem alimentação): não responde (NACK). */
  offline: boolean;
  start(): void;
  write(byte: number): void;
  read(): number;
}

/** Registradores com ponteiro autoincrementado. */
abstract class RegisterDevice implements I2CDevice {
  offline = false;
  protected regs = new Uint8Array(256);
  private ptr = 0;
  private first = true;
  constructor(readonly address: number) {}
  start(): void {
    this.first = true;
  }
  write(byte: number): void {
    if (this.first) {
      this.ptr = byte;
      this.first = false;
      this.onPointer(byte);
    } else {
      this.onWrite(this.ptr, byte);
      this.ptr = (this.ptr + 1) & 0xff;
    }
  }
  read(): number {
    const v = this.onRead(this.ptr);
    this.ptr = (this.ptr + 1) & 0xff;
    return v;
  }
  protected onPointer(_reg: number): void {}
  protected onWrite(reg: number, v: number): void {
    this.regs[reg] = v;
  }
  protected onRead(reg: number): number {
    return this.regs[reg]!;
  }
}

/**
 * BMP280. Coeficientes de calibração: o exemplo do datasheet (Bosch BST-BMP280-DS001,
 * §8.1, tabela 16). Os valores crus são obtidos invertendo, por bisseção, as fórmulas
 * inteiras de compensação do §8.2 — o firmware reconstrói exatamente a pressão simulada.
 */
export class Bmp280 extends RegisterDevice {
  static readonly CAL = { T1: 27504, T2: 26435, T3: -1000, P1: 36477, P2: -10685, P3: 3024, P4: 2855, P5: 140, P6: -7, P7: 15500, P8: -14600, P9: 6000 };
  /** Leitura atual (atualizada pelo chamador a cada passo). */
  reading: Reading | null = null;
  constructor(address: number) {
    super(address);
    this.regs[0xd0] = 0x58;
    const c = Bmp280.CAL;
    const vals = [c.T1, c.T2, c.T3, c.P1, c.P2, c.P3, c.P4, c.P5, c.P6, c.P7, c.P8, c.P9];
    vals.forEach((v, i) => {
      const u = v & 0xffff;
      this.regs[0x88 + 2 * i] = u & 0xff;
      this.regs[0x89 + 2 * i] = u >> 8;
    });
    this.latch(0x80000, 0x80000);
  }
  /** Modo normal ou forçado ativo. */
  get running(): boolean {
    return (this.regs[0xf4]! & 3) !== 0;
  }
  /** Tempo de medição típico, s (datasheet §3.8.1: 1 + 2·T_os + 2·P_os + 0,5 ms). */
  get measureTime(): number {
    const os = (x: number) => (x === 0 ? 0 : 1 << (x - 1));
    const ctrl = this.regs[0xf4]!;
    return (1 + 2 * os((ctrl >> 5) & 7) + 2 * os((ctrl >> 2) & 7) + 0.5) / 1000;
  }
  protected override onWrite(reg: number, v: number): void {
    if (reg === 0xe0 && v === 0xb6) {
      this.regs[0xf4] = 0;
      this.regs[0xf5] = 0;
      return;
    }
    if (reg === 0xf4 || reg === 0xf5) this.regs[reg] = v;
  }
  protected override onPointer(reg: number): void {
    if (reg === 0xf7 && this.running && this.reading) {
      const t = Bmp280.rawTemperature(this.reading.temperature - 273.15);
      const p = Bmp280.rawPressure(this.reading.pressure, t.tFine);
      this.latch(p, t.raw);
    }
  }
  private latch(p: number, t: number): void {
    this.regs[0xf7] = (p >> 12) & 0xff;
    this.regs[0xf8] = (p >> 4) & 0xff;
    this.regs[0xf9] = (p & 0xf) << 4;
    this.regs[0xfa] = (t >> 12) & 0xff;
    this.regs[0xfb] = (t >> 4) & 0xff;
    this.regs[0xfc] = (t & 0xf) << 4;
  }
  /** §8.2: t_fine e T em centésimos de °C a partir do valor cru. */
  static compT(adc: number): { tFine: number; t100: number } {
    const c = Bmp280.CAL;
    const v1 = (((adc >> 3) - (c.T1 << 1)) * c.T2) >> 11;
    const d = (adc >> 4) - c.T1;
    const v2 = (((d * d) >> 12) * c.T3) >> 14;
    const tFine = v1 + v2;
    return { tFine, t100: (tFine * 5 + 128) >> 8 };
  }
  /** §8.2: pressão em Pa·256 (Q24.8), com aritmética de 64 bits. */
  static compP(adc: number, tFine: number): number {
    const c = Bmp280.CAL;
    const B = BigInt;
    let v1 = B(tFine) - 128000n;
    let v2 = v1 * v1 * B(c.P6);
    v2 = v2 + ((v1 * B(c.P5)) << 17n);
    v2 = v2 + (B(c.P4) << 35n);
    v1 = ((v1 * v1 * B(c.P3)) >> 8n) + ((v1 * B(c.P2)) << 12n);
    v1 = (((1n << 47n) + v1) * B(c.P1)) >> 33n;
    if (v1 === 0n) return 0;
    let p = 1048576n - B(adc);
    p = (((p << 31n) - v2) * 3125n) / v1;
    v1 = (B(c.P9) * (p >> 13n) * (p >> 13n)) >> 25n;
    v2 = (B(c.P8) * p) >> 19n;
    p = ((p + v1 + v2) >> 8n) + (B(c.P7) << 4n);
    return Number(p);
  }
  static rawTemperature(celsius: number): { raw: number; tFine: number } {
    let lo = 0;
    let hi = (1 << 20) - 1;
    const target = Math.round(celsius * 100);
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (Bmp280.compT(mid).t100 < target) lo = mid + 1;
      else hi = mid;
    }
    return { raw: lo, tFine: Bmp280.compT(lo).tFine };
  }
  static rawPressure(pa: number, tFine: number): number {
    // A pressão compensada diminui com o valor cru.
    let lo = 0;
    let hi = (1 << 20) - 1;
    const target = pa * 256;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (Bmp280.compP(mid, tFine) > target) lo = mid + 1;
      else hi = mid;
    }
    return lo;
  }
}

/**
 * MPU-6050. Registradores: InvenSense RM-MPU-6000A-00 rev 4.2 (WHO_AM_I 0x75 = 0x68,
 * PWR_MGMT_1 0x6B começa dormindo, GYRO_CONFIG 0x1B, ACCEL_CONFIG 0x1C, dados 0x3B–0x48
 * em complemento de 2, big-endian; temperatura = valor/340 + 36,53 °C). Eixo x do sensor
 * alinhado ao eixo do foguete. Satura no fundo de escala.
 */
export class Mpu6050 extends RegisterDevice {
  reading: Reading | null = null;
  constructor(address: number) {
    super(address);
    this.regs[0x75] = 0x68;
    this.regs[0x6b] = 0x40;
  }
  get awake(): boolean {
    return (this.regs[0x6b]! & 0x40) === 0;
  }
  protected override onWrite(reg: number, v: number): void {
    if (reg === 0x6b && v & 0x80) {
      this.regs.fill(0, 0x00, 0x75);
      this.regs[0x6b] = 0x40;
      return;
    }
    if (reg !== 0x75) this.regs[reg] = v;
  }
  protected override onPointer(reg: number): void {
    if (reg >= 0x3b && reg <= 0x48 && this.awake && this.reading) this.sample(this.reading);
  }
  private sample(r: Reading): void {
    const g0 = 9.80665;
    const aFs = 2 << ((this.regs[0x1c]! >> 3) & 3);
    const gFs = 250 << ((this.regs[0x1b]! >> 3) & 3);
    const sat = (x: number) => Math.max(-32768, Math.min(32767, Math.round(x)));
    const put = (reg: number, v: number) => {
      const u = v & 0xffff;
      this.regs[reg] = u >> 8;
      this.regs[reg + 1] = u & 0xff;
    };
    r.accel.forEach((a, i) => put(0x3b + 2 * i, sat((a / g0 / aFs) * 32768)));
    put(0x41, sat((r.temperature - 273.15 - 36.53) * 340));
    r.gyro.forEach((w, i) => put(0x43 + 2 * i, sat((((w * 180) / Math.PI) / gFs) * 32768)));
  }
}

/** Sentença NMEA GGA (NMEA 0183 v4.10, §GGA) com soma de verificação. */
export function nmeaGga(tSec: number, latDeg: number, lonDeg: number, altM: number, fix: boolean, sats: number): string {
  const hh = Math.floor(tSec / 3600) % 24;
  const mm = Math.floor(tSec / 60) % 60;
  const ss = tSec % 60;
  const ll = (v: number, deg: number, pos: string, neg: string) => {
    const a = Math.abs(v);
    const d = Math.floor(a);
    const m = (a - d) * 60;
    return `${String(d).padStart(deg, "0")}${m.toFixed(4).padStart(7, "0")},${v >= 0 ? pos : neg}`;
  };
  const time = `${String(hh).padStart(2, "0")}${String(mm).padStart(2, "0")}${ss.toFixed(2).padStart(5, "0")}`;
  const body = fix
    ? `GPGGA,${time},${ll(latDeg, 2, "N", "S")},${ll(lonDeg, 3, "E", "W")},1,${String(sats).padStart(2, "0")},1.0,${altM.toFixed(1)},M,0.0,M,,`
    : `GPGGA,${time},,,,,0,00,99.9,,M,,M,,`;
  let x = 0;
  for (const ch of body) x ^= ch.charCodeAt(0);
  return `$${body}*${x.toString(16).toUpperCase().padStart(2, "0")}\r\n`;
}
