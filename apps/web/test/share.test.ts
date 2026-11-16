import { describe, expect, it } from "vitest";
import type { Design } from "@astra/core-js";
import { decodeDesign, designFromHash, encodeDesign } from "../src/share";

describe("link compartilhável", () => {
  it("ida e volta, caracteres seguros para URL", () => {
    const d = { name: "Ação ✓ çé", parts: Array.from({ length: 50 }, (_, i) => ({ id: `p${i}`, position: i / 7 })) } as unknown as Design;
    const s = encodeDesign(d);
    expect(s).toMatch(/^[\w-]+$/);
    expect(decodeDesign(s)).toEqual(d);
    expect(designFromHash(`#p=${s}`)).toEqual(d);
    expect(designFromHash("#p=lixo!")).toBeNull();
    expect(designFromHash("")).toBeNull();
  });
});
