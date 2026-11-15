use crate::Rocket;
use astra_dynamics::flight::{EventKind, Launch, Nav};
use astra_env::Planet;
use astra_propulsion::WaterRocket;

/// Números principais de um voo.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FlightSummary {
    /// Apogeu acima do solo, m.
    pub apogee_agl: f64,
    /// Instante do apogeu, s.
    pub apogee_time: f64,
    /// Velocidade ao sair do trilho, m/s.
    pub rail_exit_speed: f64,
    /// Margem estática ao sair do trilho, calibres.
    pub rail_exit_margin: f64,
    /// Velocidade máxima, m/s.
    pub max_speed: f64,
    /// Mach máximo.
    pub max_mach: f64,
    /// Pressão dinâmica máxima, Pa.
    pub max_q: f64,
    /// Duração do voo até o impacto, s.
    pub flight_time: f64,
    /// Velocidade no impacto, m/s.
    pub impact_speed: f64,
    /// Distância horizontal do local de lançamento ao impacto, m.
    pub landing_distance: f64,
    /// Azimute do impacto visto do local de lançamento, rad a partir do norte.
    pub landing_bearing: f64,
    /// Maior força de cada paraquedas (N), amostrada no intervalo de saída.
    pub opening_loads: Vec<(String, f64)>,
}

impl FlightSummary {
    /// Resume um voo a partir das amostras `(t, nav)` e dos eventos `(tipo, t, nav)`.
    /// Distância e azimute do pouso usam o plano tangente no local de lançamento
    /// (erro relativo da ordem de distância/raio do planeta).
    #[must_use]
    pub fn new(
        rocket: &Rocket,
        planet: &Planet,
        launch: &Launch,
        samples: &[(f64, Nav)],
        events: &[(EventKind, f64, Nav)],
    ) -> Self {
        let mut s = Self::default();
        let find = |k: EventKind| events.iter().find(|e| e.0 == k);
        if let Some(e) = find(EventKind::Apogee) {
            s.apogee_agl = e.2.alt - launch.ground_h;
            s.apogee_time = e.1;
        } else {
            s.apogee_agl =
                samples.iter().map(|x| x.1.alt).fold(f64::MIN, f64::max) - launch.ground_h;
        }
        if let Some(e) = find(EventKind::RailExit) {
            s.rail_exit_speed = e.2.speed;
            s.rail_exit_margin = rocket.static_margin(e.1, e.2.mach);
        }
        for (_, n) in samples {
            s.max_speed = s.max_speed.max(n.speed);
            s.max_mach = s.max_mach.max(n.mach);
            s.max_q = s.max_q.max(n.q_dyn);
        }
        if let Some(e) = find(EventKind::Impact) {
            s.flight_time = e.1;
            s.impact_speed = e.2.speed;
            let north = (e.2.lat - launch.site.lat) * planet.re;
            let east = (e.2.lon - launch.site.lon) * planet.re * launch.site.lat.cos();
            s.landing_distance = north.hypot(east);
            s.landing_bearing = east.atan2(north).rem_euclid(core::f64::consts::TAU);
        }
        for (i, p) in rocket.parachutes.iter().enumerate() {
            if let Some(fire) = find(EventKind::Trigger(i)) {
                let peak = samples
                    .iter()
                    .filter(|x| x.0 >= fire.1)
                    .map(|x| x.1.q_dyn * p.drag_area(x.0 - fire.1))
                    .fold(0.0, f64::max);
                s.opening_loads.push((p.name.clone(), peak));
            }
        }
        s
    }
}

/// Volume de água que maximiza `apogee` para a garrafa `base`, por busca da seção áurea
/// entre 5% e 90% do volume da garrafa. Supõe um único máximo nessa faixa, o que vale
/// para o foguete de água comum (pouca água: pouco impulso; muita: peso e pouca energia
/// armazenada no ar). Devolve (volume em m³, apogeu).
/// Fonte do método: Kiefer, "Sequential minimax search for a maximum" (1953).
pub fn optimal_water_volume(
    base: &WaterRocket,
    apogee: impl Fn(&WaterRocket) -> f64,
) -> (f64, f64) {
    let eval = |frac: f64| {
        apogee(&WaterRocket {
            water_volume: frac * base.bottle_volume,
            ..*base
        })
    };
    let phi = 0.5 * (5.0_f64.sqrt() - 1.0);
    let (mut a, mut b) = (0.05, 0.90);
    let (mut c, mut d) = (b - phi * (b - a), a + phi * (b - a));
    let (mut fc, mut fd) = (eval(c), eval(d));
    for _ in 0..30 {
        if fc > fd {
            (b, d, fd) = (d, c, fc);
            c = b - phi * (b - a);
            fc = eval(c);
        } else {
            (a, c, fc) = (c, d, fd);
            d = a + phi * (b - a);
            fd = eval(d);
        }
    }
    let frac = 0.5 * (a + b);
    (frac * base.bottle_volume, eval(frac))
}
