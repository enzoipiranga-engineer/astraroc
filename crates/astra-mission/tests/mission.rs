//! Missões orbitais: física, orçamento de Δv e comparação com números públicos.

use astra_mission::*;
use astra_propulsion::engine_library;

fn f9(payload: f64) -> Mission {
    templates::mission(templates::falcon9(payload), 200e3)
}

/// ∫T/m dt = Δ|v| + perdas: a identidade fecha com a integração por trapézios.
#[test]
fn dv_budget_closes() {
    let o = simulate(&f9(12_000.0));
    assert!(o.reached_orbit);
    let d = o.dv;
    let lhs = d.delivered - d.steering_loss - d.drag_loss - d.gravity_loss;
    assert!(
        (lhs / d.achieved - 1.0).abs() < 0.01,
        "{lhs} vs {}",
        d.achieved
    );
    assert!(
        d.ideal > d.delivered,
        "o ideal no vácuo supera o entregue (contrapressão e aceleração reduzida)"
    );
    assert!(
        d.gravity_loss > 500.0 && d.gravity_loss < 2000.0,
        "perda gravitacional {}",
        d.gravity_loss
    );
    assert!(
        d.drag_loss > 5.0 && d.drag_loss < 300.0,
        "perda por arrasto {}",
        d.drag_loss
    );
    // Lançamento para leste a 28,5° N: inclinação ≈ latitude; velocidade circular a 200 km.
    let orb = o.orbit.unwrap();
    assert!((orb.inc_deg - 28.56).abs() < 0.3, "i {}", orb.inc_deg);
    assert!(orb.e < 0.002 && (orb.perigee_alt - 200e3).abs() < 10e3);
    assert!((o.dv.earth_rotation - 465.1 * 28.5618_f64.to_radians().cos()).abs() < 5.0);
}

/// Δv ideal de Tsiolkovsky somado por estágio, à mão para o Falcon 9 com 10 t.
#[test]
fn ideal_dv_by_hand() {
    let v = templates::falcon9(10_000.0);
    let g0 = 9.806_65;
    let m2_0: f64 = 10_000.0 + 4_000.0 + 107_500.0;
    let dv2 = 348.0 * g0 * (m2_0 / (m2_0 - 107_500.0)).ln();
    let m1_0: f64 = m2_0 + 25_600.0 + 411_000.0 + 1_900.0;
    let dv1 = 311.0 * g0 * (m1_0 / (m1_0 - 411_000.0)).ln();
    assert!((v.ideal_dv() - (dv1 + dv2)).abs() < 1e-6);
}

/// Carga útil máxima a 200 km (busca por bisseção) contra a referência pública de 22,8 t
/// do Falcon 9 descartável. Tolerância de ±25%: a massa dos estágios é aproximada e a
/// referência é para uma órbita um pouco diferente.
#[test]
fn falcon9_max_payload_matches_public_figure() {
    let (mut lo, mut hi) = (10_000.0, 35_000.0);
    assert!(simulate(&f9(lo)).reached_orbit);
    assert!(!simulate(&f9(hi)).reached_orbit);
    for _ in 0..7 {
        let mid = 0.5 * (lo + hi);
        if simulate(&f9(mid)).reached_orbit {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    println!(
        "Falcon 9 descartável: carga máxima a 200 km ≈ {:.1} t",
        lo / 1000.0
    );
    assert!(
        lo > 0.75 * 22_800.0 && lo < 1.25 * 22_800.0,
        "carga máxima {lo}"
    );
}

#[test]
fn starship_reaches_orbit_with_100_t() {
    let o = simulate(&templates::mission(templates::starship(100_000.0), 200e3));
    assert!(o.reached_orbit, "{:?}", o.warnings);
    assert!(o.events.iter().any(|e| e.message.contains("hot staging")));
    // Sem queima de retorno (fase 10), o Super Heavy cai no mar, a algumas centenas de
    // quilômetros, bem antes da órbita.
    let sh = o.bodies.iter().find(|b| b.name == "Super Heavy").unwrap();
    let land = sh.landing.unwrap();
    let d = land[2].hypot(land[3]);
    println!("Super Heavy cai a {:.0} km", d / 1000.0);
    assert!(d > 50e3 && d < 2_000e3, "{d}");
}

/// Falha de um motor no primeiro estágio aos 30 s: o Falcon 9 ainda chega à órbita com
/// carga reduzida (a queima fica mais longa e a margem cai).
#[test]
fn engine_out_tolerance() {
    let nominal = simulate(&f9(10_000.0));
    let mut m = f9(10_000.0);
    m.vehicle.stages[0].engine_out = Some(EngineOut {
        after: 30.0,
        engines: 1,
    });
    let o = simulate(&m);
    assert!(o
        .events
        .iter()
        .any(|e| e.kind == "engineOut" && (e.t - 30.0).abs() < 1e-6));
    assert!(o.reached_orbit, "{:?}", o.warnings);
    let sep = |o: &MissionOutput| o.events.iter().find(|e| e.kind == "stageSep").unwrap().t;
    assert!(
        sep(&o) > sep(&nominal) + 5.0,
        "com 8 motores a queima dura mais"
    );
    assert!(o.propellant_margin < nominal.propellant_margin);
}

/// Boosters laterais: separam antes do fim do núcleo e caem por conta própria; o limite
/// de pressão dinâmica é respeitado.
#[test]
fn boosters_and_max_q_limit() {
    let mut v = templates::falcon9(20_000.0);
    v.name = "Falcon 9 com boosters".into();
    let merlin = engine_library()
        .into_iter()
        .find(|e| e.name == "Merlin 1D")
        .unwrap();
    v.boosters = Some(Booster {
        stage: Stage {
            name: "Booster".into(),
            dry_mass: 6_000.0,
            propellant: 60_000.0,
            engines: vec![(merlin, 2)],
            diameter: 1.8,
            length: 20.0,
            max_q: None,
            max_accel: None,
            engine_out: None,
            reserve: 0.0,
        },
        count: 2,
    });
    let o = simulate(&templates::mission(v, 200e3));
    let sep = o
        .events
        .iter()
        .find(|e| e.kind == "boosterSep")
        .expect("separação dos boosters");
    let core = o.events.iter().find(|e| e.kind == "stageSep").unwrap();
    assert!(sep.t < core.t);
    assert_eq!(
        o.bodies
            .iter()
            .filter(|b| b.name.starts_with("Booster"))
            .count(),
        2
    );
    assert!(o.reached_orbit, "{:?}", o.warnings);
    assert!(
        o.max_q.0 < 32_000.0 * 1.15,
        "max-Q {} acima do limite de 32 kPa",
        o.max_q.0
    );
}
