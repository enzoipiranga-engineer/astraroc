/**
 * Idioma da interface (português ou inglês).
 *
 * O texto é escrito em português no código e no núcleo. Em inglês, uma camada traduz o que
 * já está na tela: cada nó de texto e os atributos placeholder, title e aria-label, além
 * das caixas de alerta. Frases fixas vêm do dicionário; mensagens com números e nomes no
 * meio (avisos do núcleo, eventos do voo) casam com moldes em que {} marca a lacuna.
 */
import { EN } from "./i18n-en";

export type Lang = "pt" | "en";
const KEY = "astra.lang";

let lang: Lang = (() => {
  try {
    const s = localStorage.getItem(KEY);
    if (s === "en" || s === "pt") return s;
  } catch {
    /* sem armazenamento */
  }
  return typeof navigator !== "undefined" && !navigator.language.toLowerCase().startsWith("pt") ? "en" : "pt";
})();

export const currentLang = () => lang;
/** Locale para números e datas. */
export const locale = () => (lang === "en" ? "en-US" : "pt-BR");

const exact = new Map<string, string>();
const patterns: { re: RegExp; out: string }[] = [];
for (const [pt, en] of Object.entries(EN)) {
  if (pt.includes("{}")) {
    const parts = pt.split("{}").map((p) => p.replace(/[.*+?^$()|[\]\\]/g, "\\$&").replace(/\s+/g, "\\s+"));
    patterns.push({ re: new RegExp(`^${parts.join("(.+?)")}$`, "s"), out: en });
  } else exact.set(norm(pt), en);
}
// Moldes mais longos (mais específicos) primeiro.
patterns.sort((a, b) => b.re.source.length - a.re.source.length);

function norm(s: string): string {
  return s.replace(/\s+/g, " ").trim();
}

const cache = new Map<string, string | null>();

/** Traduz um texto (sem os espaços das pontas). Devolve null se não houver tradução. */
export function translate(text: string, depth = 0): string | null {
  const key = norm(text);
  if (!/[A-Za-zÀ-ú]{2,}/.test(key)) return null;
  const hit = exact.get(key);
  if (hit !== undefined) return hit;
  // "velocidade" usa a tradução de "Velocidade", em minúscula.
  const cap = exact.get(key.charAt(0).toUpperCase() + key.slice(1));
  if (cap !== undefined && key.charAt(0) !== key.charAt(0).toUpperCase()) return cap.charAt(0).toLowerCase() + cap.slice(1);
  const c = cache.get(key);
  if (c !== undefined) return c;
  let out: string | null = null;
  for (const p of patterns) {
    const m = p.re.exec(key);
    if (!m) continue;
    const args = m.slice(1);
    out = p.out.replace(/\{(\d+)\}/g, (_, i: string) => {
      const a = args[Number(i)] ?? "";
      return (depth < 3 && translate(a, depth + 1)) || a;
    });
    break;
  }
  if (out === null) {
    // "Nome (id)": traduz o nome e mantém o parêntese final.
    const m = /^(.+?)\s(\([^()]*\))$/.exec(key);
    if (m) {
      const t = translate(m[1]!, depth + 1);
      if (t !== null) out = `${t} ${m[2]}`;
    }
  }
  if (out === null && key.includes(" → ")) {
    const parts = key.split(" → ");
    const t = parts.map((x) => translate(x, depth + 1));
    if (t.some((x) => x !== null)) out = t.map((x, i) => x ?? parts[i]).join(" → ");
  }
  if (out === null && depth === 0) {
    // Frases compostas: traduz cada frase separada por ". " ou ": ".
    const pieces = key.split(/(?<=[.:;!?])\s+/);
    if (pieces.length > 1) {
      const t = pieces.map((p) => translate(p, depth + 1));
      if (t.some((x) => x !== null)) out = t.map((x, i) => x ?? pieces[i]).join(" ");
    }
  }
  if (cache.size > 5000) cache.clear();
  cache.set(key, out);
  return out;
}

/** Tradução para uso direto no código (devolve o original sem tradução). */
export const tr = (s: string) => (lang === "en" ? (translate(s) ?? s) : s);

// Nós de texto: original em português e o último valor traduzido que escrevemos.
const original = new WeakMap<Node, string>();
const written = new WeakMap<Node, string>();
const ATTRS = ["placeholder", "title", "aria-label"];
const originalAttr = new WeakMap<Element, Map<string, string>>();

function skip(n: Node): boolean {
  const el = n.nodeType === 1 ? (n as Element) : n.parentElement;
  return !!el?.closest("textarea, pre.console, [data-no-translate], script, style");
}

function doText(n: Text) {
  if (skip(n)) return;
  const v = n.nodeValue ?? "";
  if (lang === "pt") {
    // Volta ao original só se o texto ainda é o que nós escrevemos.
    const o = original.get(n);
    if (o !== undefined && written.get(n) === v) {
      written.delete(n);
      n.nodeValue = o;
    }
    return;
  }
  if (written.get(n) === v) return;
  original.set(n, v);
  const t = translate(v);
  if (t === null) return;
  const lead = /^\s*/.exec(v)![0];
  const trail = /\s*$/.exec(v)![0];
  const nv = lead + t + trail;
  written.set(n, nv);
  if (nv !== v) n.nodeValue = nv;
}

function doAttrs(el: Element) {
  if (skip(el)) return;
  let map = originalAttr.get(el);
  for (const a of ATTRS) {
    const v = el.getAttribute(a);
    if (v === null) continue;
    if (!map) originalAttr.set(el, (map = new Map()));
    if (lang === "pt") {
      const o = map.get(a);
      if (o !== undefined && v !== o) el.setAttribute(a, o);
      continue;
    }
    if (map.get(`${a}:out`) === v) continue;
    map.set(a, v);
    const t = translate(v);
    if (t !== null) {
      map.set(`${a}:out`, t);
      el.setAttribute(a, t);
    }
  }
}

function walk(root: Node) {
  if (root.nodeType === 3) return doText(root as Text);
  if (root.nodeType !== 1) return;
  doAttrs(root as Element);
  const it = (root.ownerDocument ?? document).createTreeWalker(root, NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT);
  for (let n = it.nextNode(); n; n = it.nextNode()) {
    if (n.nodeType === 3) doText(n as Text);
    else doAttrs(n as Element);
  }
}

let observer: MutationObserver | null = null;
const nativeAlert = typeof window !== "undefined" ? window.alert.bind(window) : null;
const nativeConfirm = typeof window !== "undefined" ? window.confirm.bind(window) : null;
const nativePrompt = typeof window !== "undefined" ? window.prompt.bind(window) : null;

/** Liga a tradução da página (chame uma vez na inicialização). */
export function startI18n() {
  document.documentElement.lang = lang === "en" ? "en" : "pt-BR";
  window.alert = (m?: unknown) => nativeAlert!(typeof m === "string" ? tr(m) : (m as string));
  window.confirm = (m?: string) => nativeConfirm!(m === undefined ? m : tr(m));
  window.prompt = (m?: string, d?: string) => nativePrompt!(m === undefined ? m : tr(m), d);
  observer?.disconnect();
  observer = new MutationObserver((muts) => {
    for (const m of muts) {
      if (m.type === "characterData") doText(m.target as Text);
      else if (m.type === "attributes") doAttrs(m.target as Element);
      else m.addedNodes.forEach(walk);
    }
  });
  observer.observe(document.body, { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: ATTRS });
  walk(document.body);
}

/** Troca o idioma e atualiza a página inteira. */
export function setLang(l: Lang) {
  lang = l;
  try {
    localStorage.setItem(KEY, l);
  } catch {
    /* sem armazenamento */
  }
  document.documentElement.lang = l === "en" ? "en" : "pt-BR";
  cache.clear();
  walk(document.body);
}

/** Traduz um documento à parte (janelas de relatório para impressão). */
export function translateDocument(doc: Document) {
  if (lang !== "en") return;
  doc.documentElement.lang = "en";
  doc.title = tr(doc.title);
  walk(doc.body);
}
