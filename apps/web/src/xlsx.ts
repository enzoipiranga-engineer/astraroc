/**
 * Planilha .xlsx mínima (Office Open XML, ECMA-376 parte 1): uma pasta de trabalho com
 * várias abas, números como números e textos inline. Compactada com fflate.
 */
import { strToU8, zipSync } from "fflate";

export type Cell = string | number | null;
export interface Sheet {
  name: string;
  rows: Cell[][];
}

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

/** Nome de coluna: 0 → A, 25 → Z, 26 → AA. */
export function colName(i: number): string {
  let s = "";
  for (let n = i + 1; n > 0; n = Math.floor((n - 1) / 26)) s = String.fromCharCode(65 + ((n - 1) % 26)) + s;
  return s;
}

function sheetXml(rows: Cell[][]): string {
  const body = rows
    .map((r, ri) => {
      const cells = r
        .map((c, ci) => {
          const ref = `${colName(ci)}${ri + 1}`;
          if (c === null || c === "") return "";
          if (typeof c === "number") return Number.isFinite(c) ? `<c r="${ref}"><v>${c}</v></c>` : "";
          return `<c r="${ref}" t="inlineStr"><is><t xml:space="preserve">${esc(c)}</t></is></c>`;
        })
        .join("");
      return `<row r="${ri + 1}">${cells}</row>`;
    })
    .join("");
  return `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>${body}</sheetData></worksheet>`;
}

/** Monta o arquivo .xlsx. Nomes de aba: até 31 caracteres, sem []:*?/\. */
export function xlsx(sheets: Sheet[]): Uint8Array {
  const names = sheets.map((s, i) => s.name.replace(/[[\]:*?/\\]/g, " ").slice(0, 31) || `Aba ${i + 1}`);
  const files: Record<string, Uint8Array> = {
    "[Content_Types].xml": strToU8(
      `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>${sheets.map((_, i) => `<Override PartName="/xl/worksheets/sheet${i + 1}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>`).join("")}</Types>`,
    ),
    "_rels/.rels": strToU8(`<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>`),
    "xl/workbook.xml": strToU8(
      `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets>${names.map((n, i) => `<sheet name="${esc(n)}" sheetId="${i + 1}" r:id="rId${i + 1}"/>`).join("")}</sheets></workbook>`,
    ),
    "xl/_rels/workbook.xml.rels": strToU8(
      `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">${sheets.map((_, i) => `<Relationship Id="rId${i + 1}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet${i + 1}.xml"/>`).join("")}</Relationships>`,
    ),
  };
  sheets.forEach((s, i) => (files[`xl/worksheets/sheet${i + 1}.xml`] = strToU8(sheetXml(s.rows))));
  return zipSync(files);
}

/** CSV com separador ";" e vírgula decimal (padrão do Excel em português). */
export function csvBr(rows: Cell[][]): string {
  const cell = (c: Cell) => {
    if (c === null) return "";
    if (typeof c === "number") return Number.isFinite(c) ? String(c).replace(".", ",") : "";
    return /[;"\n]/.test(c) ? `"${c.replace(/"/g, '""')}"` : c;
  };
  return "﻿" + rows.map((r) => r.map(cell).join(";")).join("\r\n");
}
