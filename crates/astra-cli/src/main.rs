//! Linha de comando do núcleo: voos de exemplo (Calisto, garrafa PET) e um voo 3-DOF de
//! empuxo constante com telemetria em CSV.
//!
//! Uso: `astra-cli --m0 1.2 --thrust 60 --burn 1.6 --mdot 0.03 --cda 0.004 [opções]`.
//! Todas as grandezas em SI, exceto ângulos e coordenadas, em graus.

use astra_dynamics::flight::{simulate3, Config3, ConstantVehicle, Launch, Rail};
use astra_dynamics::flight::{simulate6, Config6, EventKind};
use astra_dynamics::ode::Options;
use astra_env::{AtmosphereModel, Geodetic, Planet};
use astra_math::Vec3;
use astra_propulsion::WaterRocket;
use astra_vehicle::{optimal_water_volume, templates, FlightSummary, Rocket};
use std::process::ExitCode;

/// Voa um foguete montado no 6-DOF (Terra WGS 84, atmosfera padrão) e resume o voo.
fn fly(rocket: &Rocket, launch: Launch, t_max: f64) -> Result<FlightSummary, String> {
    let cfg = Config6 {
        planet: Planet::EARTH,
        atmosphere: AtmosphereModel::Ussa1976 { delta_t: 0.0 },
        vehicle: rocket,
        launch,
        m0: rocket.mass(0.0),
        initial_omega_body: Vec3::ZERO,
        t_max,
        output_dt: 0.02,
        ode: Options {
            rtol: 1e-8,
            atol: 1e-8,
            h_max: 0.5,
            ..Options::default()
        },
        start: None,
        wind: astra_env::Wind::calm(),
    };
    let res = simulate6(&cfg).map_err(|e| e.to_string())?;
    let samples: Vec<_> = res.samples.iter().map(|s| (s.t, s.nav)).collect();
    let events: Vec<_> = res.events.iter().map(|e| (e.0, e.1.t, e.1.nav)).collect();
    for (kind, t, nav) in &events {
        let name = match kind {
            EventKind::RailExit => "saída do trilho".to_owned(),
            EventKind::Burnout => "fim da queima".to_owned(),
            EventKind::Apogee => "apogeu".to_owned(),
            EventKind::Impact => "impacto".to_owned(),
            EventKind::Trigger(i) => format!("abre {}", rocket.parachutes[*i].name),
            EventKind::Limit(i) => format!("limite {i} excedido"),
        };
        println!(
            "  {t:8.2} s  {name:<16} alt {:8.1} m  v {:6.1} m/s",
            nav.alt - launch.ground_h,
            nav.speed
        );
    }
    Ok(FlightSummary::new(
        rocket,
        &Planet::EARTH,
        &launch,
        &samples,
        &events,
    ))
}

fn print_summary(s: &FlightSummary) {
    println!(
        "apogeu            {:.1} m acima do solo, em {:.1} s",
        s.apogee_agl, s.apogee_time
    );
    println!(
        "saída do trilho   {:.1} m/s, margem estática {:.2} calibres",
        s.rail_exit_speed, s.rail_exit_margin
    );
    println!(
        "máximos           {:.1} m/s, Mach {:.2}, pressão dinâmica {:.0} Pa",
        s.max_speed, s.max_mach, s.max_q
    );
    println!(
        "pouso             {:.1} m/s após {:.1} s, a {:.0} m, azimute {:.0}°",
        s.impact_speed,
        s.flight_time,
        s.landing_distance,
        s.landing_bearing.to_degrees()
    );
    for (name, f) in &s.opening_loads {
        println!("paraquedas        {name}: força máxima {f:.0} N");
    }
}

fn site(lat: f64, lon: f64, h: f64, rail: f64, elev: f64) -> Launch {
    Launch {
        site: Geodetic {
            lat: lat.to_radians(),
            lon: lon.to_radians(),
            h,
        },
        ground_h: h,
        rail: Rail {
            length: rail,
            elevation: elev.to_radians(),
            azimuth: 0.0,
            friction: 0.0,
        },
        initial_speed: 0.0,
        theta0: 0.0,
    }
}

const HELP: &str = "\
astra-cli calisto                      voa o foguete Calisto (6-DOF) e resume o voo
astra-cli agua [--garrafa L] [--agua L] [--pressao bar] [--lat --lon --alt] [--otimizar]
                                       foguete de garrafa PET (6-DOF)
astra-cli --m0 ... [opções]            voo 3-DOF de empuxo constante (CSV em stdout,
                                       eventos em stderr):

  --m0 <kg>        massa inicial                     (obrigatório)
  --thrust <N>     empuxo no vácuo                   (padrão 0)
  --burn <s>       duração da queima                 (padrão 0)
  --mdot <kg/s>    vazão de propelente               (padrão 0)
  --exit-area <m2> área de saída da tubeira          (padrão 0)
  --cda <m2>       área de arrasto Cd·A              (padrão 0)
  --lat <graus>    latitude                          (padrão 0)
  --lon <graus>    longitude                         (padrão 0)
  --alt <m>        altitude do local                 (padrão 0)
  --rail <m>       comprimento do trilho             (padrão 0)
  --elev <graus>   elevação do trilho                (padrão 90)
  --az <graus>     azimute do trilho                 (padrão 0)
  --friction <->   atrito do trilho                  (padrão 0)
  --speed <m/s>    velocidade inicial no eixo        (padrão 0)
  --delta-t <K>    desvio de temperatura ISA         (padrão 0)
  --vacuum         sem atmosfera
  --tmax <s>       duração máxima                    (padrão 600)
  --dt <s>         intervalo de saída                (padrão 0.05)
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{HELP}");
        return ExitCode::SUCCESS;
    }
    if args[0] == "calisto" {
        println!(
            "Calisto (exemplo do RocketPy), Spaceport America, trilho de 5,2 m a 85°, sem vento"
        );
        return match fly(
            &templates::calisto(),
            site(32.990_254, -106.974_998, 1400.0, 5.2, 85.0),
            900.0,
        ) {
            Ok(s) => {
                print_summary(&s);
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("erro: {e}");
                ExitCode::FAILURE
            }
        };
    }
    let get = |name: &str, default: Option<f64>| -> Result<f64, String> {
        match args.iter().position(|a| a == name) {
            Some(i) => args
                .get(i + 1)
                .and_then(|v| v.parse().ok())
                .ok_or_else(|| format!("valor numérico ausente ou inválido para {name}")),
            None => default.ok_or_else(|| format!("parâmetro obrigatório ausente: {name}")),
        }
    };
    if args[0] == "agua" {
        let run = || -> Result<(), String> {
            let base = WaterRocket::pet_2l();
            let w = WaterRocket {
                bottle_volume: get("--garrafa", Some(2.0))? * 1e-3,
                water_volume: get("--agua", Some(0.66))? * 1e-3,
                gauge_pressure: get("--pressao", Some(4.0))? * 1e5,
                ..base
            };
            let burn = w.simulate();
            for warn in &burn.warnings {
                println!("AVISO: {warn}");
            }
            println!(
                "motor: fase de água {:.0} ms, fase de ar {:.0} ms, empuxo máximo {:.0} N, impulso {:.1} N·s",
                burn.water_phase_time * 1e3,
                burn.air_phase_time * 1e3,
                burn.motor.peak_thrust(),
                burn.motor.total_impulse()
            );
            let l = site(
                get("--lat", Some(-23.5))?,
                get("--lon", Some(-46.6))?,
                get("--alt", Some(760.0))?,
                0.5,
                88.0,
            );
            print_summary(&fly(&templates::pet_bottle(&w), l, 120.0)?);
            if args.iter().any(|a| a == "--otimizar") {
                let apogee = |w: &WaterRocket| {
                    let r = templates::pet_bottle(w);
                    let cfg = Config3 {
                        planet: Planet::EARTH,
                        atmosphere: AtmosphereModel::Ussa1976 { delta_t: 0.0 },
                        vehicle: &r,
                        launch: l,
                        m0: r.mass(0.0),
                        t_max: 120.0,
                        output_dt: 0.05,
                        ode: Options {
                            h_max: 0.5,
                            ..Options::default()
                        },
                        start: None,
                        wind: astra_env::Wind::calm(),
                    };
                    simulate3(&cfg)
                        .ok()
                        .and_then(|x| x.event(EventKind::Apogee).map(|a| a.nav.alt - l.ground_h))
                        .unwrap_or(0.0)
                };
                let (v, a) = optimal_water_volume(&w, apogee);
                println!(
                    "água ótima        {:.2} L ({:.0}% da garrafa): apogeu {a:.1} m",
                    v * 1e3,
                    100.0 * v / w.bottle_volume
                );
            }
            Ok(())
        };
        return match run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("erro: {e}");
                ExitCode::FAILURE
            }
        };
    }
    let run = || -> Result<(), String> {
        let vehicle = ConstantVehicle {
            thrust_vac: get("--thrust", Some(0.0))?,
            exit_area: get("--exit-area", Some(0.0))?,
            mdot: get("--mdot", Some(0.0))?,
            burn_time: get("--burn", Some(0.0))?,
            cd_a: get("--cda", Some(0.0))?,
        };
        let alt = get("--alt", Some(0.0))?;
        let cfg = Config3 {
            planet: Planet::EARTH,
            atmosphere: if args.iter().any(|a| a == "--vacuum") {
                AtmosphereModel::Vacuum
            } else {
                AtmosphereModel::Ussa1976 {
                    delta_t: get("--delta-t", Some(0.0))?,
                }
            },
            vehicle: &vehicle,
            launch: Launch {
                site: Geodetic {
                    lat: get("--lat", Some(0.0))?.to_radians(),
                    lon: get("--lon", Some(0.0))?.to_radians(),
                    h: alt,
                },
                ground_h: alt,
                rail: Rail {
                    length: get("--rail", Some(0.0))?,
                    elevation: get("--elev", Some(90.0))?.to_radians(),
                    azimuth: get("--az", Some(0.0))?.to_radians(),
                    friction: get("--friction", Some(0.0))?,
                },
                initial_speed: get("--speed", Some(0.0))?,
                theta0: 0.0,
            },
            m0: get("--m0", None)?,
            t_max: get("--tmax", Some(600.0))?,
            output_dt: get("--dt", Some(0.05))?,
            ode: Options {
                h_max: 1.0,
                ..Options::default()
            },
            start: None,
            wind: astra_env::Wind::calm(),
        };
        let res = simulate3(&cfg).map_err(|e| e.to_string())?;
        println!("t_s,alt_m,speed_m_s,vertical_speed_m_s,mach,q_dyn_Pa,lat_deg,lon_deg,mass_kg");
        for s in &res.samples {
            let n = s.nav;
            println!(
                "{:.4},{:.4},{:.4},{:.4},{:.5},{:.3},{:.8},{:.8},{:.5}",
                s.t,
                n.alt,
                n.speed,
                n.vertical_speed,
                n.mach,
                n.q_dyn,
                n.lat.to_degrees(),
                n.lon.to_degrees(),
                s.mass
            );
        }
        for (kind, s) in &res.events {
            eprintln!(
                "{kind:?}: t = {:.3} s, alt = {:.2} m, v = {:.2} m/s",
                s.t, s.nav.alt, s.nav.speed
            );
        }
        Ok(())
    };
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("erro: {e}\n\n{HELP}");
            ExitCode::FAILURE
        }
    }
}
