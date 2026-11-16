import { describe, expect, it } from "vitest";
import { parseTacview, pitchYaw, toTacview } from "../src/tacview";
import { parseTrack } from "../src/geo";
import type { SimOutput } from "@astra/core-js";

describe("Tacview", () => {
  it("lê o exemplo da documentação, com campos repetidos e UTC", () => {
    const csv = "Time,Longitude,Latitude,Altitude,Roll (deg),Pitch (deg),Yaw (deg)\n0.00,-116.6847222,32.72722222,34,-0.11536,-0.14832,295.76\n4.00,-116.6847222,32.72722222,34,0.36255,-0.26917,295.25\n8.00,-116.685,,35,0.42297,-0.2417,297.27";
    const p = parseTacview(csv)!;
    expect(p).toHaveLength(3);
    expect(p[2]).toEqual({ t: 8, lat: 32.72722222, lon: -116.685, alt: 35 });
    const utc = parseTacview("Time;Latitude;Longitude;Altitude\n2011-06-02T05:00:47.13Z;41.5;41.6;2000\n2011-06-02T05:00:48.63Z;41.5;41.6;2010")!;
    expect(utc[1]!.t).toBeCloseTo(1.5, 9);
    expect(parseTacview("lat,lon\n1,2")).toBeNull();
    expect(parseTrack("voo.csv", csv)).toHaveLength(3);
  });

  it("arfagem e guinada no referencial local e exportação", () => {
    // No equador, longitude 0: cima = +x, leste = +y, norte = +z.
    expect(pitchYaw([1, 0, 0], 0, 0)[0]).toBeCloseTo(90, 9);
    const [p, y] = pitchYaw([0, Math.SQRT1_2, Math.SQRT1_2], 0, 0);
    expect(p).toBeCloseTo(0, 9);
    expect(y).toBeCloseTo(45, 9);
    const row = [0.5, 10, 50, 49, 0.15, 1500, 0, 0, 2, 1.2, 0, 0, 480, 1, 0, 0];
    const out = { bodies: [{ id: 0, name: "Foguete", parts: [], models: [], parent: null, samples: [row], fate: "", landing: null }], events: [], warnings: [] } as unknown as SimOutput;
    const f = toTacview(out, "Equipe X");
    expect(f[0]!.name).toBe("Foguete (Equipe X) [Orange].csv");
    expect(f[0]!.text.split("\n")[1]).toBe("0.500,0.0000000,0.0000000,480.00,,90.00,0.00,2.00,50.00,0.150");
    expect(parseTacview(f[0]!.text)![0]).toEqual({ t: 0, lat: 0, lon: 0, alt: 480 });
  });
});
