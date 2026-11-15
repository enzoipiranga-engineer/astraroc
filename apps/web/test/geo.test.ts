import { describe, expect, it } from "vitest";
import { bearingName, parseTrack, parseWindCsv, powerLawProfile, profileFromResponse, windUrl } from "../src/geo";

describe("vento real (formato da Open-Meteo)", () => {
  // Formato da resposta horária documentado em open-meteo.com/en/docs (variáveis por hora).
  const hourly = {
    time: ["2026-09-03T13:00", "2026-09-03T14:00"],
    wind_speed_10m: [2, 3],
    wind_direction_10m: [80, 90],
    wind_speed_80m: [4, 5],
    wind_direction_80m: [85, 95],
    wind_speed_1000hPa: [3, 4],
    wind_direction_1000hPa: [90, 90],
    geopotential_height_1000hPa: [120, 125],
    wind_speed_850hPa: [8, 9],
    wind_direction_850hPa: [100, 110],
    geopotential_height_850hPa: [1530, 1535],
    wind_speed_500hPa: [null, 20],
    wind_direction_500hPa: [null, 250],
    geopotential_height_500hPa: [null, 5880],
  };

  it("pega a hora certa e converte níveis de pressão em altura acima do solo", () => {
    const p = profileFromResponse(hourly as never, new Date("2026-09-03T14:20:00Z"), 470);
    expect(p[0]).toEqual({ agl: 0, speed: 3, fromDeg: 90 });
    expect(p.find((x) => x.agl === 80)).toEqual({ agl: 80, speed: 5, fromDeg: 95 });
    // 1000 hPa está abaixo do solo (125 m < 470 m) e é descartado.
    expect(p.some((x) => x.speed === 4 && x.fromDeg === 90)).toBe(false);
    expect(p.find((x) => x.fromDeg === 110)!.agl).toBeCloseTo(1535 - 470);
    expect(p.at(-1)!.agl).toBeCloseTo(5880 - 470);
    expect(p.map((x) => x.agl)).toEqual([...p.map((x) => x.agl)].sort((a, b) => a - b));
  });

  it("usa o arquivo histórico para datas antigas", () => {
    expect(windUrl(-22, -49, new Date("2023-05-01T12:00:00Z"))).toContain("historical-forecast-api");
    expect(windUrl(-22, -49, new Date(Date.now() + 86_400_000))).toContain("api.open-meteo.com");
  });
});

describe("leitores", () => {
  it("CSV de vento com vírgula ou ponto e vírgula", () => {
    expect(parseWindCsv("altura,vel,dir\n0;2;90\n500,6,120\n")).toEqual([
      { agl: 0, speed: 2, fromDeg: 90 },
      { agl: 500, speed: 6, fromDeg: 120 },
    ]);
    expect(() => parseWindCsv("nada")).toThrow();
  });

  it("lei de potência", () => {
    const p = powerLawProfile(5, 10, 1 / 7, 45, 1000);
    expect(p.at(-1)).toEqual({ agl: 1000, speed: 5 * 100 ** (1 / 7), fromDeg: 45 });
  });

  it("GPX, KML e CSV de GPS", () => {
    const gpx = `<gpx><trk><trkseg><trkpt lat="-22.1" lon="-49.3"><ele>470</ele></trkpt><trkpt lat="-22.2" lon="-49.4"><ele>900</ele></trkpt></trkseg></trk></gpx>`;
    expect(parseTrack("voo.gpx", gpx)).toEqual([{ lat: -22.1, lon: -49.3, alt: 470 }, { lat: -22.2, lon: -49.4, alt: 900 }]);
    const kml = `<kml><coordinates>-49.3,-22.1,470 -49.4,-22.2,900</coordinates></kml>`;
    expect(parseTrack("voo.kml", kml)[1]).toEqual({ lat: -22.2, lon: -49.4, alt: 900 });
    expect(parseTrack("voo.csv", "lat,lon,alt\n-22.1,-49.3,470\n")).toEqual([{ lat: -22.1, lon: -49.3, alt: 470 }]);
  });

  it("azimute em palavras", () => {
    expect(bearingName(100, 100).name).toBe("nordeste");
    expect(bearingName(-1, 0).name).toBe("sul");
    expect(bearingName(0, -5).deg).toBeCloseTo(270);
  });
});
