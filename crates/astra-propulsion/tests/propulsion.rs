//! Testes da propulsão: arquivos de motor, balística interna e foguete de água.

use astra_propulsion::*;
use core::f64::consts::PI;

const M1670: &str = include_str!("../../../data/motors/Cesaroni_M1670.eng");

/// Curva comercial: Cesaroni Pro75 M1670. O arquivo reproduz o cabeçalho e o impulso
/// total fica na classe M (5120 a 10240 N·s), com empuxo médio próximo dos 1670 N da
/// designação.
#[test]
fn commercial_motor_file_is_reproduced() {
    let m = &parse_eng(M1670).unwrap()[0];
    assert_eq!(m.name, "M1670-BS");
    assert_eq!(m.manufacturer, "CTI");
    assert!((m.diameter - 0.075).abs() < 1e-12 && (m.length - 0.757).abs() < 1e-12);
    assert!((m.initial_prop_mass() - 3.101).abs() < 1e-12);
    assert!((m.dry_mass - (5.231 - 3.101)).abs() < 1e-12);
    assert_eq!(m.burn_time(), 3.9);
    assert_eq!(m.peak_thrust(), 2200.0);
    assert_eq!(m.impulse_class(), Some('M'));
    // Trapézios feitos à mão sobre os 15 pontos do arquivo: 6026,35 N·s.
    assert!(
        (m.total_impulse() - 6026.35).abs() < 0.01,
        "impulso {}",
        m.total_impulse()
    );
    assert!(
        (m.average_thrust() / 1670.0 - 1.0).abs() < 0.08,
        "média {}",
        m.average_thrust()
    );
    assert!((m.specific_impulse() - 198.2).abs() < 0.5);
    // Interpolação e massa.
    assert_eq!(m.thrust_ref(0.0), 0.0);
    assert!((m.thrust_ref(0.75) - 1992.0).abs() < 1e-9);
    assert_eq!(m.thrust_ref(4.0), 0.0);
    assert_eq!(m.prop_mass(3.9), 0.0);
    assert!((m.prop_mass(1.0) / 3.101 - (1.0 - 1809.85 / 6026.35)).abs() < 1e-9);
}

/// A vazão integrada trecho a trecho devolve exatamente a massa da tabela.
#[test]
fn mass_flow_integrates_to_propellant_mass() {
    let m = &parse_eng(M1670).unwrap()[0];
    let bp = m.breakpoints();
    let total: f64 = bp
        .windows(2)
        .map(|w| m.mdot(0.5 * (w[0] + w[1])) * (w[1] - w[0]))
        .sum();
    assert!((total - 3.101).abs() < 1e-12);
}

#[test]
fn eng_round_trip_and_errors() {
    let m = &parse_eng(M1670).unwrap()[0];
    let again = &parse_eng(&m.to_eng()).unwrap()[0];
    assert!((again.total_impulse() - m.total_impulse()).abs() < 0.01);
    assert_eq!(again.points.len(), m.points.len());
    assert!(parse_eng("; só comentário\n").is_err());
    assert!(parse_eng("X 10 20 0 0.1 0.2 Y\n0.1 abc\n")
        .unwrap_err()
        .contains("linha 2"));
    assert!(parse_eng("X 10 20 0 0.1 0.2 Y\n0.2 5\n0.1 0\n")
        .unwrap_err()
        .contains("fora de ordem"));
}

#[test]
fn rse_file_is_parsed() {
    let rse = r#"<engine-database><engine-list>
      <engine mfg="Teste" code="T100" dia="29." len="124." initWt="130." propWt="60." Type="reloadable">
        <data>
          <eng-data t="0." f="0." m="60." cg="62."/>
          <eng-data t="0.1" f="120." m="55." cg="62."/>
          <eng-data t="1.0" f="100." m="5." cg="62."/>
          <eng-data t="1.1" f="0." m="0." cg="62."/>
        </data>
      </engine></engine-list></engine-database>"#;
    let m = &parse_rse(rse).unwrap()[0];
    assert_eq!(
        (m.name.as_str(), m.manufacturer.as_str()),
        ("T100", "Teste")
    );
    assert!((m.diameter - 0.029).abs() < 1e-12 && (m.dry_mass - 0.070).abs() < 1e-12);
    assert!((m.total_impulse() - (6.0 + 99.0 + 5.0)).abs() < 1e-9);
    assert!(
        (m.prop_mass(0.1) - 0.055).abs() < 1e-12,
        "massa vem do arquivo"
    );
    assert!(parse_rse("<nada/>").is_err());
}

fn bates() -> BatesMotor {
    BatesMotor {
        propellant: Propellant::knsb(),
        grains: 4,
        outer_diameter: 0.050,
        core_diameter: 0.020,
        grain_length: 0.090,
        throat_diameter: 0.014,
        exit_diameter: 0.035,
        efficiency: 0.85,
        dry_mass: 1.2,
        case_diameter: 0.060,
        case_length: 0.45,
    }
}

/// Grão BATES: área de queima, Kn, pressão de equilíbrio e conservação de massa.
#[test]
fn bates_internal_ballistics() {
    let b = bates();
    // Área inicial à mão: 4·[π·0,02·0,09 + (π/2)(0,05² − 0,02²)].
    let a0 = 4.0 * (PI * 0.02 * 0.09 + 0.5 * PI * (0.0025 - 0.0004));
    assert!((b.burn_area(0.0) - a0).abs() < 1e-12);
    assert!((b.web() - 0.015).abs() < 1e-15);
    let burn = b.simulate();
    let at = 0.25 * PI * 0.014_f64.powi(2);
    assert!((burn.kn[0] - a0 / at).abs() < 1e-9);
    // Equilíbrio: a pressão devolvida satisfaz ρ·A_b·r(p) = p·A_t/c* em todos os pontos.
    let p = &b.propellant;
    for (kn, pc) in burn
        .kn
        .iter()
        .zip(&burn.chamber_pressure)
        .take(400)
        .step_by(40)
    {
        let generated = p.density * kn * p.burn_rate_at(*pc);
        assert!(
            (generated / (pc / p.c_star) - 1.0).abs() < 1e-9,
            "Kn {kn}: pc {pc}"
        );
    }
    let pmax = burn.chamber_pressure.iter().copied().fold(0.0, f64::max);
    assert!(
        pmax > 1.5e6 && pmax < 9e6,
        "pressão máxima plausível para KNSB: {pmax}"
    );
    // Massa: geométrica = tabela; Isp entregue na faixa do KNSB (110 a 140 s).
    let m = &burn.motor;
    assert!((m.initial_prop_mass() - b.propellant_mass()).abs() < 1e-12);
    assert_eq!(m.prop_mass(m.burn_time()), 0.0);
    let isp = m.specific_impulse();
    assert!(isp > 105.0 && isp < 140.0, "Isp {isp}");
    // Perfil BATES com L/D ≈ 1,8: Kn quase neutro (variação menor que 25%).
    let (kmin, kmax) = burn.kn[..400]
        .iter()
        .fold((f64::MAX, 0.0_f64), |a, &k| (a.0.min(k), a.1.max(k)));
    assert!(kmax / kmin < 1.25, "Kn de {kmin} a {kmax}");
    // Correção de altitude: no vácuo o empuxo sobe de p_ref·A_e.
    let t = 0.5 * m.burn_time();
    assert!((m.thrust(t, 0.0) - m.thrust_ref(t) - 101_325.0 * m.exit_area).abs() < 1e-9);
}

/// Lei de Saint-Robert e equilíbrio com um só trecho: p = (Kn·ρ·a·c*)^(1/(1−n)).
#[test]
fn single_range_chamber_pressure_formula() {
    let p = Propellant {
        name: "teste".into(),
        density: 1700.0,
        c_star: 1500.0,
        gamma: 1.2,
        burn_rate: vec![BurnRateRange {
            p_min: 0.0,
            a_mm_s_mpa: 5.0,
            n: 0.3,
        }],
    };
    let pc = p.chamber_pressure(200.0);
    let want = (200.0 * 1700.0 * 1500.0 * 5.0e-9_f64).powf(1.0 / 0.7) * 1e6;
    assert!((pc / want - 1.0).abs() < 1e-12);
    assert!((p.burn_rate_at(4e6) - 5.0e-3 * 4.0_f64.powf(0.3)).abs() < 1e-15);
}

/// Reservatório de ar muito grande: pressão constante, então F = 2·A·Δp (Cd = 1).
#[test]
fn water_thrust_at_constant_pressure() {
    let w = WaterRocket {
        bottle_volume: 10.0,
        water_volume: 1e-3,
        discharge_coeff: 1.0,
        bottle_diameter: 2.0,
        ..WaterRocket::pet_2l()
    };
    let b = w.simulate();
    let want = 2.0 * 0.25 * PI * 0.0215_f64.powi(2) * 4.0e5;
    assert!((b.motor.points[0].thrust / want - 1.0).abs() < 1e-6);
    assert!((b.motor.thrust_ref(0.5 * b.water_phase_time) / want - 1.0).abs() < 1e-3);
    // Tempo para esvaziar 1 L a velocidade constante v = √(2Δp/ρ).
    let v = (2.0 * 4.0e5 / 998.2_f64).sqrt();
    assert!(
        (b.water_phase_time / (1e-3 / (0.25 * PI * 0.0215_f64.powi(2) * v)) - 1.0).abs() < 1e-3
    );
}

/// Garrafa de 2 L: pressão ao fim da água pela adiabática, impulso da fase de água
/// igual a ∫√(2ρΔp) dV (Bernoulli, Cd = 1, bocal pequeno), massa de ar expelida pela
/// expansão isentrópica até a pressão ambiente.
#[test]
fn water_rocket_two_phase_burn() {
    let w = WaterRocket {
        discharge_coeff: 1.0,
        bottle_diameter: 10.0,
        ..WaterRocket::pet_2l()
    };
    let b = w.simulate();
    assert!(b.warnings.is_empty());
    let (v0, vb): (f64, f64) = (2.0e-3 - 0.66e-3, 2.0e-3);
    let p0 = 4.0e5 + 101_325.0;
    let p1 = p0 * (v0 / vb).powf(1.4);
    assert!((b.pressure_at_water_end / p1 - 1.0).abs() < 1e-9);

    // Quadratura independente (ponto médio, 200 mil passos) do impulso da fase de água.
    let n = 200_000;
    let dv = (vb - v0) / f64::from(n);
    let want: f64 = (0..n)
        .map(|i| {
            let v = v0 + (f64::from(i) + 0.5) * dv;
            (2.0 * 998.2 * (p0 * (v0 / v).powf(1.4) - 101_325.0)).sqrt() * dv
        })
        .sum();
    let m = &b.motor;
    let water_impulse: f64 = m
        .points
        .windows(2)
        .filter(|p| p[1].t <= b.water_phase_time + 1e-12)
        .map(|p| 0.5 * (p[0].thrust + p[1].thrust) * (p[1].t - p[0].t))
        .sum();
    assert!(
        (water_impulse / want - 1.0).abs() < 2e-3,
        "{water_impulse} vs {want}"
    );

    // Fase de ar.
    let t1 = 293.15 * (v0 / vb).powf(0.4);
    let m1 = p1 * vb / (287.052_87 * t1);
    let air = m1 * (1.0 - (101_325.0 / p1).powf(1.0 / 1.4));
    assert!((m.initial_prop_mass() - (0.66e-3 * 998.2 + air)).abs() < 1e-9);
    assert!(
        b.air_phase_time > 0.005 && b.air_phase_time < 0.2,
        "fase de ar {}",
        b.air_phase_time
    );
    // Empuxo do ar no primeiro instante: bocal bloqueado, à mão.
    let a = 0.25 * PI * 0.0215_f64.powi(2);
    let mdot = a * p1 * (1.4 / (287.052_87 * t1)).sqrt() * (2.0_f64 / 2.4).powf(3.0);
    let f_air = mdot * (1.4 * 287.052_87 * t1 / 1.2).sqrt()
        + (p1 * (2.0_f64 / 2.4).powf(3.5) - 101_325.0) * a;
    let first_air = m.points.iter().find(|p| p.t > b.water_phase_time).unwrap();
    assert!(
        (first_air.thrust / f_air - 1.0).abs() < 1e-9,
        "{} vs {f_air}",
        first_air.thrust
    );
    assert_eq!(m.points.last().unwrap().thrust, 0.0);
    assert!(m
        .points
        .windows(2)
        .all(|p| p[1].t > p[0].t && p[1].prop_mass <= p[0].prop_mass + 1e-12));
    // Ordem de grandeza conhecida: dezenas de N·s e Isp de poucos segundos.
    assert!(
        m.total_impulse() > 8.0 && m.total_impulse() < 40.0,
        "impulso {}",
        m.total_impulse()
    );
}

#[test]
fn water_rocket_warnings() {
    let over = WaterRocket {
        gauge_pressure: 9.0e5,
        ..WaterRocket::pet_2l()
    }
    .simulate();
    assert!(over
        .warnings
        .iter()
        .any(|w| w.contains("limite de segurança")));
    // Água demais para pouca pressão: sobra água.
    let weak = WaterRocket {
        water_volume: 1.9e-3,
        gauge_pressure: 0.5e5,
        ..WaterRocket::pet_2l()
    }
    .simulate();
    assert!(weak.water_left > 0.5 && weak.air_phase_time == 0.0);
    assert!(weak.warnings.iter().any(|w| w.contains("peso morto")));
    assert!((weak.motor.dry_mass - weak.water_left).abs() < 1e-12);
}
