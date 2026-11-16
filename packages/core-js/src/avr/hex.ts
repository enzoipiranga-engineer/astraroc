/**
 * Leitor de Intel HEX (registros 00 dados, 01 fim, 02 segmento estendido, 04 endereço
 * linear estendido). Fonte: Intel, "Hexadecimal Object File Format Specification", rev. A.
 */
export function parseHex(text: string, flashBytes = 32768): Uint16Array {
  const bytes = new Uint8Array(flashBytes);
  let base = 0;
  let used = 0;
  for (const [n, raw] of text.split(/\r?\n/).entries()) {
    const line = raw.trim();
    if (!line) continue;
    if (line[0] !== ":") throw new Error(`HEX inválido na linha ${n + 1}: não começa com ':'.`);
    const rec = line.slice(1).match(/../g)?.map((h) => parseInt(h, 16)) ?? [];
    if (rec.some((x) => Number.isNaN(x)) || rec.length < 5) throw new Error(`HEX inválido na linha ${n + 1}.`);
    const len = rec[0]!;
    if (rec.length !== len + 5) throw new Error(`HEX inválido na linha ${n + 1}: tamanho errado.`);
    const sum = rec.reduce((a, b) => a + b, 0) & 0xff;
    if (sum !== 0) throw new Error(`HEX inválido na linha ${n + 1}: soma de verificação errada.`);
    const addr = (rec[1]! << 8) | rec[2]!;
    const type = rec[3]!;
    const data = rec.slice(4, 4 + len);
    if (type === 0) {
      const a0 = base + addr;
      if (a0 + len > flashBytes) throw new Error(`O programa não cabe na memória (${flashBytes} bytes).`);
      bytes.set(data, a0);
      used = Math.max(used, a0 + len);
    } else if (type === 1) break;
    else if (type === 2) base = ((data[0]! << 8) | data[1]!) << 4;
    else if (type === 4) base = ((data[0]! << 8) | data[1]!) << 16;
  }
  if (used === 0) throw new Error("O arquivo HEX não tem dados.");
  return new Uint16Array(bytes.buffer);
}
