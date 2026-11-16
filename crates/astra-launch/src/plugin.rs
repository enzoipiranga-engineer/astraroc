//! Lançadores da comunidade: pacote JSON declarativo, versionado, com parâmetros,
//! expressões seguras e testes próprios.

use crate::expr::Expr;
use crate::guided::{guided, Guide, Push};
use crate::{Base, LocalEnv, Payload};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Parâmetro de um plugin.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamSpec {
    /// Nome (usado nas expressões).
    pub name: String,
    /// Valor padrão.
    pub value: f64,
    /// Unidade SI declarada.
    pub unit: String,
    /// Mínimo da faixa de validade.
    pub min: f64,
    /// Máximo da faixa de validade.
    pub max: f64,
    /// Explicação.
    #[serde(default)]
    pub help: String,
}

/// Teste declarado pelo autor do plugin.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginTest {
    /// Nome.
    pub name: String,
    /// Massa do foguete inerte usado no teste, kg.
    pub mass: f64,
    /// Parâmetros sobrepostos.
    #[serde(default)]
    pub values: BTreeMap<String, f64>,
    /// Velocidade de liberação mínima esperada, m/s.
    pub min_speed: f64,
    /// Máxima, m/s.
    pub max_speed: f64,
}

/// Pacote de plugin. Hoje cobre lançadores de guia retilínea com força arbitrária
/// (expressão em x, v, t, m e parâmetros).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSpec {
    /// Identificador único (ex.: "autor/nome").
    pub id: String,
    /// Nome.
    pub name: String,
    /// Versão semântica.
    pub version: String,
    /// Autor.
    pub author: String,
    /// Descrição.
    pub description: String,
    /// Faixa de validade do modelo, em texto.
    pub validity: String,
    /// Parâmetros.
    pub params: Vec<ParamSpec>,
    /// Comprimento da guia (expressão), m.
    pub length: String,
    /// Elevação (expressão), graus.
    pub elevation_deg: String,
    /// Azimute (expressão), graus.
    #[serde(default = "zero")]
    pub azimuth_deg: String,
    /// Massa do carrinho (expressão), kg.
    #[serde(default = "zero")]
    pub carriage_mass: String,
    /// Coeficiente de atrito (expressão).
    #[serde(default = "zero")]
    pub friction: String,
    /// Força ao longo da guia (expressão em x, v, t, m e parâmetros), N.
    pub force: String,
    /// Acender o motor no início (senão, na liberação).
    #[serde(default)]
    pub ignite_on_guide: bool,
    /// Modelo 3D de referência para a animação ("rail", "tube", "ramp").
    #[serde(default)]
    pub model: String,
    /// Testes.
    #[serde(default)]
    pub tests: Vec<PluginTest>,
}

fn zero() -> String {
    "0".into()
}

/// Resultado da verificação automática de um plugin.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCheck {
    /// Passou em tudo.
    pub ok: bool,
    /// Mensagens (erros e confirmações).
    pub messages: Vec<String>,
}

struct Inert(f64);
impl Payload for Inert {
    fn mass(&self, _: f64, _: f64) -> f64 {
        self.0
    }
    fn thrust(&self, _: f64, _: f64, _: f64) -> f64 {
        0.0
    }
    fn drag_area(&self) -> f64 {
        0.002
    }
    fn length(&self) -> f64 {
        1.0
    }
    fn diameter(&self) -> f64 {
        0.05
    }
}

impl PluginSpec {
    fn consts(&self, values: &BTreeMap<String, f64>) -> BTreeMap<String, f64> {
        let mut c: BTreeMap<String, f64> = self
            .params
            .iter()
            .map(|p| {
                (
                    p.name.clone(),
                    values.get(&p.name).copied().unwrap_or(p.value),
                )
            })
            .collect();
        c.insert("g".into(), 9.806_65);
        c.insert("pi".into(), core::f64::consts::PI);
        c
    }

    /// Converte em guia para os valores de parâmetros dados.
    ///
    /// # Errors
    /// Expressão inválida, variável desconhecida ou parâmetro fora da faixa.
    pub fn to_guide(&self, values: &BTreeMap<String, f64>) -> Result<Guide, String> {
        let consts = self.consts(values);
        for p in &self.params {
            let v = consts[&p.name];
            if v < p.min || v > p.max {
                return Err(format!(
                    "parâmetro {} = {v} fora da faixa de validade [{}, {}] {}",
                    p.name, p.min, p.max, p.unit
                ));
            }
        }
        let eval_const = |label: &str, src: &str| -> Result<f64, String> {
            let e = Expr::parse(src).map_err(|e| format!("{label}: {e}"))?;
            let mut vars = Vec::new();
            e.vars(&mut vars);
            if let Some(v) = vars.iter().find(|v| !consts.contains_key(*v)) {
                return Err(format!("{label}: variável desconhecida \"{v}\""));
            }
            let x = e.eval(&consts);
            if x.is_finite() {
                Ok(x)
            } else {
                Err(format!("{label}: resultado não finito"))
            }
        };
        let force = Expr::parse(&self.force).map_err(|e| format!("força: {e}"))?;
        let mut vars = Vec::new();
        force.vars(&mut vars);
        if let Some(v) = vars
            .iter()
            .find(|v| !consts.contains_key(*v) && !["x", "v", "t", "m"].contains(&v.as_str()))
        {
            return Err(format!("força: variável desconhecida \"{v}\""));
        }
        Ok(Guide {
            label: self.name.clone(),
            length: eval_const("comprimento", &self.length)?,
            elevation_deg: eval_const("elevação", &self.elevation_deg)?,
            azimuth_deg: eval_const("azimute", &self.azimuth_deg)?,
            carriage_mass: eval_const("massa do carrinho", &self.carriage_mass)?,
            friction: eval_const("atrito", &self.friction)?,
            carriage_cd_a: 0.0,
            push: Push::Expr {
                source: self.force.clone(),
                consts,
            },
            release_speed: None,
            ignite_at: self.ignite_on_guide.then_some(0.0),
            ignition_delay: 0.0,
            brake: 50.0,
            height: 0.0,
        })
    }

    /// Verificação automática: campos obrigatórios, unidades declaradas, expressões
    /// válidas, balanço de energia fechado (resíduo < 1e-6) com um foguete inerte em
    /// atmosfera padrão, e os testes do próprio pacote.
    #[must_use]
    pub fn check(&self) -> PluginCheck {
        let mut c = PluginCheck {
            ok: true,
            messages: Vec::new(),
        };
        let fail = |c: &mut PluginCheck, m: String| {
            c.ok = false;
            c.messages.push(m);
        };
        if self.id.is_empty() || self.name.is_empty() || self.version.split('.').count() != 3 {
            fail(&mut c, "Faltam id, nome ou versão no formato x.y.z.".into());
        }
        if self.validity.trim().is_empty() {
            fail(&mut c, "Declare a faixa de validade do modelo.".into());
        }
        for p in &self.params {
            if p.unit.trim().is_empty() {
                fail(
                    &mut c,
                    format!("O parâmetro {} não declara unidade.", p.name),
                );
            }
            if !(p.min <= p.value && p.value <= p.max) {
                fail(
                    &mut c,
                    format!("O valor padrão de {} está fora da faixa.", p.name),
                );
            }
        }
        let env = LocalEnv {
            g: 9.806_65,
            ground: 0.0,
            atm: astra_env::AtmosphereModel::Ussa1976 { delta_t: 0.0 },
            wind: astra_env::Wind::calm(),
        };
        match self.to_guide(&BTreeMap::new()) {
            Err(e) => fail(&mut c, e),
            Ok(g) => {
                let o = guided(&g, Base::default(), &Inert(5.0), &env);
                match o.release {
                    None => fail(&mut c, format!("Com os valores padrão o lançador não libera um foguete de 5 kg: {}", o.warnings.join(" "))),
                    Some(_) if o.energy.residual > 1e-6 => fail(&mut c, format!("O balanço de energia não fecha (resíduo {:.1e}): a força depende de algo além do estado?", o.energy.residual)),
                    Some(r) => c.messages.push(format!("Energia conservada (resíduo {:.1e}); libera 5 kg a {:.1} m/s.", o.energy.residual, r.vel.norm())),
                }
            }
        }
        for t in &self.tests {
            match self.to_guide(&t.values) {
                Err(e) => fail(&mut c, format!("Teste \"{}\": {e}", t.name)),
                Ok(g) => {
                    let o = guided(&g, Base::default(), &Inert(t.mass), &env);
                    let v = o.release.map_or(0.0, |r| r.vel.norm());
                    if v < t.min_speed || v > t.max_speed {
                        fail(
                            &mut c,
                            format!(
                                "Teste \"{}\" falhou: {:.2} m/s fora de [{}, {}].",
                                t.name, v, t.min_speed, t.max_speed
                            ),
                        );
                    } else {
                        c.messages
                            .push(format!("Teste \"{}\" passou: {:.2} m/s.", t.name, v));
                    }
                }
            }
        }
        c
    }
}

/// Plugins de exemplo que acompanham o ASTRA (modelos de referência para quem for
/// escrever o seu: "crie seu lançador").
#[must_use]
pub fn builtin_plugins() -> Vec<PluginSpec> {
    let p = |name: &str, value: f64, unit: &str, min: f64, max: f64, help: &str| ParamSpec {
        name: name.into(),
        value,
        unit: unit.into(),
        min,
        max,
        help: help.into(),
    };
    vec![
        PluginSpec {
            id: "astra/motor-linear".into(),
            name: "Motor linear de potência limitada".into(),
            version: "1.0.0".into(),
            author: "ASTRA".into(),
            description: "Trilho eletromagnético: força máxima até a velocidade em que a potência limita (F = min(Fmax, P/v)).".into(),
            validity: "Guia rígida e retilínea; eficiência elétrica não modelada (só potência mecânica).".into(),
            params: vec![
                p("P", 200e3, "W", 1e3, 1e8, "Potência mecânica"),
                p("Fmax", 20e3, "N", 10.0, 1e7, "Força máxima"),
                p("L", 30.0, "m", 1.0, 5e3, "Comprimento do trilho"),
                p("elev", 80.0, "graus", 0.0, 90.0, "Elevação"),
                p("mc", 20.0, "kg", 0.0, 1e5, "Massa do carrinho"),
            ],
            length: "L".into(),
            elevation_deg: "elev".into(),
            azimuth_deg: "0".into(),
            carriage_mass: "mc".into(),
            friction: "0.002".into(),
            force: "min(Fmax, P / max(v, 0.01))".into(),
            ignite_on_guide: false,
            model: "rail".into(),
            tests: vec![PluginTest { name: "5 kg sem perdas ≈ √(2·P·...)".into(), mass: 5.0, values: BTreeMap::new(), min_speed: 50.0, max_speed: 120.0 }],
        },
        PluginSpec {
            id: "astra/canhao-ar".into(),
            name: "Canhão de ar comprimido".into(),
            version: "1.0.0".into(),
            author: "ASTRA".into(),
            description: "Reservatório de ar que expande adiabaticamente atrás do foguete: F = (p0·(V0/(V0 + A·x))^1,4 − patm)·A.".into(),
            validity: "Gás ideal, expansão adiabática rápida, sem atrito do gás nem vazamento.".into(),
            params: vec![
                p("p0", 8e5, "Pa", 1.2e5, 3e7, "Pressão absoluta inicial"),
                p("V0", 0.01, "m³", 1e-4, 10.0, "Volume do reservatório"),
                p("d", 0.06, "m", 0.01, 1.0, "Diâmetro do cano"),
                p("L", 2.0, "m", 0.1, 100.0, "Comprimento do cano"),
                p("elev", 75.0, "graus", 0.0, 90.0, "Elevação"),
            ],
            length: "L".into(),
            elevation_deg: "elev".into(),
            azimuth_deg: "0".into(),
            carriage_mass: "0.05".into(),
            friction: "0.02".into(),
            force: "max(0, (p0 * (V0 / (V0 + pi*d^2/4*x))^1.4 - 101325) * pi*d^2/4)".into(),
            ignite_on_guide: false,
            model: "tube".into(),
            tests: vec![PluginTest { name: "foguete de 1 kg".into(), mass: 1.0, values: BTreeMap::new(), min_speed: 20.0, max_speed: 150.0 }],
        },
    ]
}
