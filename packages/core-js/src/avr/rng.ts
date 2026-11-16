/** Gerador determinístico SplitMix64 (Steele, Lea & Flood, OOPSLA 2014) com Box-Muller. */
export class Rng {
  private s: bigint;
  private spare: number | null = null;
  constructor(seed: number) {
    this.s = BigInt.asUintN(64, BigInt(Math.floor(seed)));
  }
  next(): number {
    this.s = BigInt.asUintN(64, this.s + 0x9e3779b97f4a7c15n);
    let z = this.s;
    z = BigInt.asUintN(64, (z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n);
    z = BigInt.asUintN(64, (z ^ (z >> 27n)) * 0x94d049bb133111ebn);
    z = z ^ (z >> 31n);
    return Number(z >> 11n) / 2 ** 53;
  }
  /** Normal padrão (Box-Muller). */
  gauss(): number {
    if (this.spare !== null) {
      const v = this.spare;
      this.spare = null;
      return v;
    }
    const u = Math.max(this.next(), 1e-300);
    const v = this.next();
    const r = Math.sqrt(-2 * Math.log(u));
    this.spare = r * Math.sin(2 * Math.PI * v);
    return r * Math.cos(2 * Math.PI * v);
  }
}
