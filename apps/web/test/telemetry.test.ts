import { describe, expect, it } from "vitest";
import { parseTelemetry } from "../src/components/AnalysisPanel";

describe("telemetria do altímetro", () => {
  it("lê CSV, zera a altitude do campo e o tempo na decolagem", () => {
    const csv = "tempo;altitude\n0;470\n0,5;470\n1;471\n1.5;480\n2;500\n3;540";
    const d = parseTelemetry(csv.replace(/0,5/, "0.5"));
    expect(d[0]).toEqual([0, 10]);
    expect(d.at(-1)).toEqual([1.5, 70]);
    expect(parseTelemetry("a,b\n1,2")).toHaveLength(1);
  });
});
