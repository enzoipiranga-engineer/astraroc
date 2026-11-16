/**
 * ATmega328P emulado com avr8js (CPU, timers 0–2, USART0, TWI, portas B/C/D), com
 * barramento I2C de dispositivos virtuais. É o chip das placas Uno, Nano e Pro Mini.
 */
import {
  AVRIOPort,
  AVRTimer,
  AVRTWI,
  AVRUSART,
  CPU,
  PinState,
  avrInstruction,
  portBConfig,
  portCConfig,
  portDConfig,
  timer0Config,
  timer1Config,
  timer2Config,
  twiConfig,
  usart0Config,
  type TWIEventHandler,
} from "avr8js";
import type { I2CDevice } from "./devices";

/** Pino da placa Arduino → (porta, bit). D0–D7 = PORTD, D8–D13 = PORTB, A0–A5 = PORTC. */
export function arduinoPin(name: string): { port: "B" | "C" | "D"; bit: number } | null {
  const d = /^D(\d+)$/.exec(name);
  if (d) {
    const n = Number(d[1]);
    if (n <= 7) return { port: "D", bit: n };
    if (n <= 13) return { port: "B", bit: n - 8 };
    return null;
  }
  const a = /^A([0-5])$/.exec(name);
  return a ? { port: "C", bit: Number(a[1]) } : null;
}

class Bus implements TWIEventHandler {
  private target: I2CDevice | null = null;
  private writing = false;
  constructor(
    private readonly twi: AVRTWI,
    readonly devices: I2CDevice[],
  ) {}
  start(): void {
    this.twi.completeStart();
  }
  stop(): void {
    this.target = null;
    this.twi.completeStop();
  }
  connectToSlave(addr: number, write: boolean): void {
    const dev = this.devices.find((d) => d.address === addr && !d.offline) ?? null;
    this.target = dev;
    this.writing = write;
    if (dev && write) dev.start();
    this.twi.completeConnect(dev !== null);
  }
  writeByte(value: number): void {
    if (this.target && this.writing && !this.target.offline) {
      this.target.write(value);
      this.twi.completeWrite(true);
    } else this.twi.completeWrite(false);
  }
  readByte(_ack: boolean): void {
    this.twi.completeRead(this.target && !this.target.offline ? this.target.read() : 0xff);
  }
}

export class Atmega328p {
  readonly cpu: CPU;
  readonly ports: Record<"B" | "C" | "D", AVRIOPort>;
  readonly usart: AVRUSART;
  readonly twi: AVRTWI;
  private readonly bus: Bus;
  /** Linha de serial completa recebida (sem \r\n). */
  onLine: ((line: string) => void) | null = null;
  private line = "";
  private readonly keep: unknown[];

  constructor(
    program: Uint16Array,
    readonly clockHz: number,
    devices: I2CDevice[],
  ) {
    this.cpu = new CPU(new Uint16Array(program), 0x800);
    this.keep = [new AVRTimer(this.cpu, timer0Config), new AVRTimer(this.cpu, timer1Config), new AVRTimer(this.cpu, timer2Config)];
    this.ports = { B: new AVRIOPort(this.cpu, portBConfig), C: new AVRIOPort(this.cpu, portCConfig), D: new AVRIOPort(this.cpu, portDConfig) };
    this.usart = new AVRUSART(this.cpu, usart0Config, clockHz);
    this.usart.onByteTransmit = (b) => {
      const ch = String.fromCharCode(b);
      if (ch === "\n") {
        this.onLine?.(this.line.replace(/\r$/, ""));
        this.line = "";
      } else if (this.line.length < 400) this.line += ch;
    };
    this.twi = new AVRTWI(this.cpu, twiConfig, clockHz);
    this.bus = new Bus(this.twi, devices);
    this.twi.eventHandler = this.bus;
  }

  /** Tempo desde que a placa ligou, s. */
  get time(): number {
    return this.cpu.cycles / this.clockHz;
  }

  /** Roda até o instante `t` (s desde que ligou). */
  runUntil(t: number): void {
    const end = t * this.clockHz;
    const cpu = this.cpu;
    while (cpu.cycles < end) {
      avrInstruction(cpu);
      cpu.tick();
    }
  }

  /** Nível de saída de um pino Arduino (true = alto). */
  pinHigh(name: string): boolean {
    const p = arduinoPin(name);
    if (!p) return false;
    return this.ports[p.port].pinState(p.bit) === PinState.High;
  }

  /** Envia bytes à serial da placa (RX), um por vez quando o receptor fica livre. */
  private rxQueue: number[] = [];
  feedSerial(text: string): void {
    for (const ch of text) this.rxQueue.push(ch.charCodeAt(0) & 0xff);
    this.pumpRx();
  }
  pumpRx(): void {
    if (this.usart.rxEnable && !this.usart.rxBusy && this.rxQueue.length) this.usart.writeByte(this.rxQueue.shift()!);
  }
  get peripherals(): unknown[] {
    return this.keep;
  }
}
