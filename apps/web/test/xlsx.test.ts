import { strFromU8, unzipSync } from "fflate";
import { describe, expect, it } from "vitest";
import { colName, csvBr, xlsx } from "../src/xlsx";

describe("planilha", () => {
  it("monta um .xlsx válido com várias abas", () => {
    const z = unzipSync(xlsx([{ name: "BOM", rows: [["Item", "Total"], ["Tubo <papel>", 12.5]] }, { name: "Receitas", rows: [["Fonte"], ["Edital"]] }]));
    expect(Object.keys(z).sort()).toEqual(["[Content_Types].xml", "_rels/.rels", "xl/_rels/workbook.xml.rels", "xl/workbook.xml", "xl/worksheets/sheet1.xml", "xl/worksheets/sheet2.xml"]);
    const s1 = strFromU8(z["xl/worksheets/sheet1.xml"]!);
    expect(s1).toContain('<c r="B2"><v>12.5</v></c>');
    expect(s1).toContain("Tubo &lt;papel&gt;");
    expect(strFromU8(z["xl/workbook.xml"]!)).toContain('name="Receitas"');
  });
  it("nomes de coluna e CSV em português", () => {
    expect([colName(0), colName(25), colName(26), colName(701)]).toEqual(["A", "Z", "AA", "ZZ"]);
    expect(csvBr([["a;b", 1.5], [null, "x"]])).toBe('﻿"a;b";1,5\r\n;x');
  });
});
