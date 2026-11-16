/**
 * Link compartilhável: o projeto vai compactado (deflate) e em base64url no fragmento do
 * endereço (#p=…). O fragmento não é enviado a nenhum servidor.
 */
import { deflateSync, inflateSync, strFromU8, strToU8 } from "fflate";
import type { Design } from "@astra/core-js";

function toB64url(b: Uint8Array): string {
  let s = "";
  for (let i = 0; i < b.length; i += 0x8000) s += String.fromCharCode(...b.subarray(i, i + 0x8000));
  return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}
function fromB64url(s: string): Uint8Array {
  const b = atob(s.replace(/-/g, "+").replace(/_/g, "/") + "===".slice((s.length + 3) % 4));
  return Uint8Array.from(b, (c) => c.charCodeAt(0));
}

export function encodeDesign(d: Design): string {
  return toB64url(deflateSync(strToU8(JSON.stringify(d)), { level: 9 }));
}

export function decodeDesign(s: string): Design {
  return JSON.parse(strFromU8(inflateSync(fromB64url(s)))) as Design;
}

/** Projeto no fragmento do endereço, se houver. */
export function designFromHash(hash: string): Design | null {
  const m = /[#&]p=([\w-]+)/.exec(hash);
  if (!m) return null;
  try {
    return decodeDesign(m[1]!);
  } catch {
    return null;
  }
}
