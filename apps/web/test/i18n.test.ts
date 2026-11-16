// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { setLang, startI18n, translate } from "../src/i18n";

describe("tradução para inglês", () => {
  it("frases fixas, moldes com lacunas, frases compostas e parêntese final", () => {
    expect(translate("Desfazer")).toBe("Undo");
    expect(translate("Saiu do trilho a 24.9 m/s.")).toBe("Left the rail at 24.9 m/s.");
    expect(translate("0.2 s: Saiu do trilho a 24.9 m/s.")).toBe("0.2 s: Left the rail at 24.9 m/s.");
    expect(translate("Ignição do 2º estágio.")).toBe("Ignition of the 2nd stage.");
    expect(translate("Barômetro BMP280 em módulo com regulador (GY-BMP280 5 V) (baro)")).toBe("BMP280 barometer on a module with regulator (GY-BMP280 5 V) (baro)");
    expect(translate("Carga de ejeção de 0.80 g gerou 1053 N e venceu os 265 N da junta: baia aberta, Paraquedas saiu.")).toBe("Ejection charge of 0.80 g produced 1053 N and beat the 265 N of the joint: bay open, Parachute came out.");
    expect(translate("12,5 m")).toBeNull();
    expect(translate("velocidade")).toBe("speed");
  });

  it("troca o texto da página e volta ao português", () => {
    document.body.innerHTML = `<div><button title="Desfazer">Desfazer</button><p> Margem estática </p><pre class="console">Massa</pre></div>`;
    setLang("en");
    startI18n();
    expect(document.querySelector("button")!.textContent).toBe("Undo");
    expect(document.querySelector("button")!.title).toBe("Undo");
    expect(document.querySelector("p")!.textContent).toBe(" Static margin ");
    expect(document.querySelector("pre")!.textContent).toBe("Massa");
    setLang("pt");
    expect(document.querySelector("button")!.textContent).toBe("Desfazer");
    expect(document.querySelector("button")!.title).toBe("Desfazer");
  });
});
