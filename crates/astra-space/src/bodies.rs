//! Corpos do Sistema Solar.
//!
//! GM e raios: NASA/JPL, "Planetary Physical Parameters" e NASA Planetary Fact Sheets
//! (valores arredondados aos dígitos mostrados). Órbitas: semieixo maior, excentricidade
//! e longitude média em J2000 da tabela 1 de E. M. Standish, "Keplerian Elements for
//! Approximate Positions of the Major Planets" (JPL), **transcritos de memória** — ver
//! docs/PENDENCIAS.md. Atmosferas exponenciais: densidade na superfície e altura de escala
//! das NASA Planetary Fact Sheets.

use serde::Serialize;

/// Atmosfera exponencial ρ = ρ0·exp(−h/H).
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpAtmosphere {
    /// Densidade na superfície, kg/m³.
    pub rho0: f64,
    /// Altura de escala, m.
    pub scale_height: f64,
    /// Constante de Sutton-Graves do gás, kg^0,5/m (Sutton & Graves, NASA TR R-376, 1971).
    pub sutton_graves: f64,
    /// Velocidade do som típica, m/s.
    pub sound_speed: f64,
}

impl ExpAtmosphere {
    /// Densidade na altitude `h`.
    #[must_use]
    pub fn density(&self, h: f64) -> f64 {
        self.rho0 * (-h.max(0.0) / self.scale_height).exp()
    }
}

/// Órbita heliocêntrica média.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HelioOrbit {
    /// Semieixo maior, m.
    pub a: f64,
    /// Excentricidade.
    pub e: f64,
    /// Longitude média em J2000, rad.
    pub l0: f64,
    /// Variação da longitude média, rad/s.
    pub l_rate: f64,
}

/// Corpo celeste.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Body {
    /// Identificador.
    pub id: &'static str,
    /// Nome.
    pub name: &'static str,
    /// Parâmetro gravitacional GM, m³/s².
    pub gm: f64,
    /// Raio médio, m.
    pub radius: f64,
    /// Corpo em torno do qual orbita.
    pub parent: Option<&'static str>,
    /// Semieixo maior em torno do pai, m.
    pub a_parent: f64,
    /// Órbita heliocêntrica (planetas).
    pub helio: Option<HelioOrbit>,
    /// Atmosfera.
    pub atmosphere: Option<ExpAtmosphere>,
}

/// Unidade astronômica, m (IAU 2012, resolução B2).
pub const AU: f64 = 149_597_870_700.0;
const DEG: f64 = core::f64::consts::PI / 180.0;
/// Século juliano, s.
const CENTURY: f64 = 36_525.0 * 86_400.0;

const fn helio(a_au: f64, e: f64, l0_deg: f64, rate_deg_cy: f64) -> Option<HelioOrbit> {
    Some(HelioOrbit {
        a: a_au * AU,
        e,
        l0: l0_deg * DEG,
        l_rate: rate_deg_cy * DEG / CENTURY,
    })
}

/// Corpos disponíveis.
pub const BODIES: [Body; 9] = [
    Body {
        id: "sol",
        name: "Sol",
        gm: 1.327_124_400_18e20,
        radius: 695_700e3,
        parent: None,
        a_parent: 0.0,
        helio: None,
        atmosphere: None,
    },
    Body {
        id: "mercurio",
        name: "Mercúrio",
        gm: 2.2032e13,
        radius: 2_439.7e3,
        parent: Some("sol"),
        a_parent: 0.387_099_27 * AU,
        helio: helio(
            0.387_099_27,
            0.205_635_93,
            252.250_323_50,
            149_472.674_111_75,
        ),
        atmosphere: None,
    },
    Body {
        id: "venus",
        name: "Vênus",
        gm: 3.248_59e14,
        radius: 6_051.8e3,
        parent: Some("sol"),
        a_parent: 0.723_335_66 * AU,
        helio: helio(
            0.723_335_66,
            0.006_776_72,
            181.979_099_50,
            58_517.815_387_29,
        ),
        atmosphere: Some(ExpAtmosphere {
            rho0: 65.0,
            scale_height: 15_900.0,
            sutton_graves: 1.896e-4,
            sound_speed: 410.0,
        }),
    },
    Body {
        id: "terra",
        name: "Terra",
        gm: 3.986_004_418e14,
        radius: 6_371.0e3,
        parent: Some("sol"),
        a_parent: 1.000_002_61 * AU,
        helio: helio(
            1.000_002_61,
            0.016_711_23,
            100.464_571_66,
            35_999.372_449_81,
        ),
        atmosphere: Some(ExpAtmosphere {
            rho0: 1.225,
            scale_height: 8_500.0,
            sutton_graves: 1.7415e-4,
            sound_speed: 320.0,
        }),
    },
    Body {
        id: "lua",
        name: "Lua",
        gm: 4.904_869_5e12,
        radius: 1_737.4e3,
        parent: Some("terra"),
        a_parent: 384_400e3,
        helio: None,
        atmosphere: None,
    },
    Body {
        id: "marte",
        name: "Marte",
        gm: 4.282_837e13,
        radius: 3_389.5e3,
        parent: Some("sol"),
        a_parent: 1.523_710_34 * AU,
        helio: helio(1.523_710_34, 0.093_394_10, -4.553_432_05, 19_140.302_684_99),
        atmosphere: Some(ExpAtmosphere {
            rho0: 0.020,
            scale_height: 11_100.0,
            sutton_graves: 1.9027e-4,
            sound_speed: 240.0,
        }),
    },
    Body {
        id: "jupiter",
        name: "Júpiter",
        gm: 1.266_865_34e17,
        radius: 69_911e3,
        parent: Some("sol"),
        a_parent: 5.202_887 * AU,
        helio: helio(5.202_887, 0.048_386_24, 34.396_440_51, 3_034.746_127_75),
        atmosphere: None,
    },
    Body {
        id: "saturno",
        name: "Saturno",
        gm: 3.793_118_7e16,
        radius: 58_232e3,
        parent: Some("sol"),
        a_parent: 9.536_675_94 * AU,
        helio: helio(9.536_675_94, 0.053_861_79, 49.954_244_23, 1_222.493_622_01),
        atmosphere: None,
    },
    Body {
        id: "tita",
        name: "Titã",
        gm: 8.978_1e12,
        radius: 2_574.7e3,
        parent: Some("saturno"),
        a_parent: 1_221_870e3,
        helio: None,
        atmosphere: Some(ExpAtmosphere {
            rho0: 5.3,
            scale_height: 40_000.0,
            sutton_graves: 1.7407e-4,
            sound_speed: 195.0,
        }),
    },
];

/// Procura um corpo pelo identificador.
#[must_use]
pub fn body(id: &str) -> Option<&'static Body> {
    BODIES.iter().find(|b| b.id == id)
}

impl Body {
    /// Velocidade de escape na superfície, m/s: √(2μ/R).
    #[must_use]
    pub fn escape_speed(&self) -> f64 {
        (2.0 * self.gm / self.radius).sqrt()
    }
    /// Gravidade na superfície, m/s².
    #[must_use]
    pub fn surface_gravity(&self) -> f64 {
        self.gm / (self.radius * self.radius)
    }
    /// Esfera de influência em torno do pai, m: r = a·(m/M)^(2/5) (Laplace; Curtis,
    /// Orbital Mechanics for Engineering Students, eq. 8.4).
    #[must_use]
    pub fn soi(&self) -> f64 {
        self.parent.and_then(body).map_or(f64::INFINITY, |p| {
            self.a_parent * (self.gm / p.gm).powf(0.4)
        })
    }
    /// Longitude heliocêntrica média no instante `t` (s desde J2000), rad.
    #[must_use]
    pub fn mean_longitude(&self, t: f64) -> Option<f64> {
        self.helio
            .map(|h| (h.l0 + h.l_rate * t).rem_euclid(core::f64::consts::TAU))
    }
}
