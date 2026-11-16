import { afterEach, describe, expect, it } from "vitest";
import { fromDisplay, labelOf, setUnitSystem, toDisplay } from "../src/units";

describe("unidades", () => {
  afterEach(() => setUnitSystem("si"));
  it("SI e imperial com ida e volta exata", () => {
    expect(toDisplay(0.0254, "mm")).toBeCloseTo(25.4, 12);
    setUnitSystem("imperial");
    expect(toDisplay(0.0254, "mm")).toBeCloseTo(1, 12);
    expect(labelOf("mm")).toBe("in");
    expect(toDisplay(0.453_592_37, "kg")).toBeCloseTo(1, 12);
    expect(toDisplay(4.448_221_615_260_5, "N")).toBeCloseTo(1, 12);
    expect(fromDisplay(toDisplay(1.234, "m"), "m")).toBeCloseTo(1.234, 12);
    expect(labelOf("deg")).toBe("°");
  });
});
