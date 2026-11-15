//! Roda um voo 3-DOF de empuxo constante e imprime a telemetria em CSV.
//!
//! Uso: `astra-cli --m0 1.2 --thrust 60 --burn 1.6 --mdot 0.03 --cda 0.004 [opções]`.
//! Todas as grandezas em SI, exceto ângulos e coordenadas, em graus.

use astra_dynamics::flight::{simulate3, Config3, ConstantVehicle, Launch, Rail};
use astra_dynamics::ode::Options;
use astra_env::{AtmosphereModel, Geodetic, Planet};
use std::process::ExitCode;

const HELP: &str = "\
astra-cli: voo 3-DOF de empuxo constante (saída CSV em stdout, eventos em stderr)

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
    let get = |name: &str, default: Option<f64>| -> Result<f64, String> {
        match args.iter().position(|a| a == name) {
            Some(i) => args
                .get(i + 1)
                .and_then(|v| v.parse().ok())
                .ok_or_else(|| format!("valor numérico ausente ou inválido para {name}")),
            None => default.ok_or_else(|| format!("parâmetro obrigatório ausente: {name}")),
        }
    };
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
