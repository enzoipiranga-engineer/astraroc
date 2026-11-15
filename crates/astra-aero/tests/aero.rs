//! Testes da aerodinâmica contra fórmulas fechadas e valores clássicos.

use astra_aero::*;
use core::f64::consts::PI;

fn nose_only(shape: NoseShape, length: f64, d: f64) -> Geometry {
    Geometry {
        parts: vec![
            BodyPart::Nose {
                shape,
                length,
                diameter: d,
            },
            BodyPart::Tube {
                length: 1.0,
                diameter: d,
            },
        ],
        fins: vec![],
        roughness: 2e-5,
        nozzle_exit_diameter: 0.0,
    }
}

/// Barrowman: todo nariz tem CNα = 2; CP do cone em 2L/3, da ogiva tangente em 0,466L,
/// do von Kármán em L/2 (volume = metade do cilindro), da parábola tangente em 7L/15
/// (volume = 8/15 do cilindro) e da elipse em L/3 (volume = 2/3).
#[test]
fn nose_cn_alpha_and_cp() {
    for (shape, frac, tol) in [
        (NoseShape::Conical, 2.0 / 3.0, 1e-4),
        (NoseShape::TangentOgive, 0.466, 2e-3),
        (NoseShape::Parabolic, 7.0 / 15.0, 1e-4),
        (NoseShape::VonKarman, 0.5, 1e-4),
        (NoseShape::Elliptical, 1.0 / 3.0, 1e-4),
    ] {
        let g = nose_only(shape, 0.5, 0.1);
        let n = g.normal_force(0.0, 0.0);
        assert!((n.cn_alpha - 2.0).abs() < 1e-9, "{shape:?}");
        assert!(
            (n.x_cp / 0.5 - frac).abs() < tol,
            "{shape:?}: {}",
            n.x_cp / 0.5
        );
    }
}

/// Transição cônica: CNα = 2[(d2/d)² − (d1/d)²] e
/// x = L/3·[1 + (1 − d1/d2)/(1 − (d1/d2)²)] (Barrowman 1967).
#[test]
fn conical_transition() {
    let (d1, d2, l) = (0.05, 0.1, 0.2);
    let g = Geometry {
        parts: vec![
            BodyPart::Tube {
                length: 0.3,
                diameter: d1,
            },
            BodyPart::Transition {
                length: l,
                d_fore: d1,
                d_aft: d2,
            },
            BodyPart::Tube {
                length: 0.3,
                diameter: d2,
            },
        ],
        fins: vec![],
        roughness: 0.0,
        nozzle_exit_diameter: 0.0,
    };
    let c = g.linear_components(0.0);
    assert_eq!(c.len(), 1);
    assert!((c[0].0 - 2.0 * (1.0 - 0.25)).abs() < 1e-9);
    let x = 0.3 + l / 3.0 * (1.0 + (1.0 - d1 / d2) / (1.0 - (d1 / d2).powi(2)));
    assert!((c[0].1 - x).abs() < 1e-6, "{} vs {x}", c[0].1);
}

/// Foguete clássico de exemplo (dimensões em polegadas): ogiva de 12,5, corpo de
/// diâmetro 5,54, três aletas com cr = 10, ct = 0, s = 5,25, recuo 9, raiz em x = 27.
/// Valor esperado calculado à mão com as equações fechadas de Barrowman:
/// (CN)F = [1 + R/(S+R)]·4N(S/d)²/(1 + √(1 + (2Lf/(Cr+Ct))²)),
/// XF = XB + Xr(Cr+2Ct)/(3(Cr+Ct)) + (Cr + Ct − CrCt/(Cr+Ct))/6.
#[test]
fn classic_three_fin_rocket_cp() {
    let (d, cr, ct, s, xr, xb, n) = (5.54, 10.0, 0.0, 5.25, 9.0, 27.0, 3.0);
    let g = Geometry {
        parts: vec![
            BodyPart::Nose {
                shape: NoseShape::TangentOgive,
                length: 12.5,
                diameter: d,
            },
            BodyPart::Tube {
                length: 24.5,
                diameter: d,
            },
        ],
        fins: vec![FinSet {
            count: 3,
            planform: FinPlanform::Trapezoid {
                root_chord: cr,
                tip_chord: ct,
                sweep: xr,
            },
            span: s,
            thickness: 0.1,
            position: xb,
            profile: FinProfile::Rounded,
        }],
        roughness: 0.0,
        nozzle_exit_diameter: 0.0,
    };
    let lf = f64::hypot(s, xr + ct / 2.0 - cr / 2.0);
    let r = d / 2.0;
    let cnf = (1.0 + r / (s + r)) * 4.0 * n * (s / d).powi(2)
        / (1.0 + (1.0 + (2.0 * lf / (cr + ct)).powi(2)).sqrt());
    let xf = xb + xr * (cr + 2.0 * ct) / (3.0 * (cr + ct)) + (cr + ct - cr * ct / (cr + ct)) / 6.0;
    let want = (2.0 * 0.466 * 12.5 + cnf * xf) / (2.0 + cnf);
    let got = g.normal_force(0.0, 0.0);
    assert!(
        (got.cn_alpha - (2.0 + cnf)).abs() < 1e-9,
        "CNα {} vs {}",
        got.cn_alpha,
        2.0 + cnf
    );
    // 0,466L é o valor arredondado de Barrowman para a ogiva; o código integra o perfil.
    assert!((got.x_cp - want).abs() < 0.03, "CP {} vs {want}", got.x_cp);
    assert!(
        (want - 24.7).abs() < 0.2,
        "ordem de grandeza do exemplo: {want}"
    );
    // Margem de 1,5 calibre com CG 1,5 diâmetro à frente do CP.
    assert!((g.static_margin(0.0, got.x_cp - 1.5 * d) - 1.5).abs() < 1e-12);
}

#[test]
fn elliptical_fin_cp_and_area() {
    let f = FinSet {
        count: 4,
        planform: FinPlanform::Ellipse { root_chord: 0.1 },
        span: 0.05,
        thickness: 0.003,
        position: 1.0,
        profile: FinProfile::Airfoil,
    };
    assert!((f.area() - PI * 0.1 * 0.05 / 4.0).abs() < 1e-15);
    assert!((f.x_cp(0.0) - 1.0288).abs() < 1e-12);
    assert!((f.x_cp(3.0) - 1.05).abs() < 1e-12);
}

/// Sustentação de corpo: com α > 0 o CP anda para a frente e CN cresce mais que linear.
#[test]
fn body_lift_moves_cp_forward_with_alpha() {
    let mut g = nose_only(NoseShape::TangentOgive, 0.3, 0.1);
    g.fins.push(FinSet {
        count: 4,
        planform: FinPlanform::Trapezoid {
            root_chord: 0.15,
            tip_chord: 0.07,
            sweep: 0.05,
        },
        span: 0.1,
        thickness: 0.003,
        position: 1.1,
        profile: FinProfile::Rounded,
    });
    let a0 = g.normal_force(0.3, 0.0);
    let a10 = g.normal_force(0.3, 10.0_f64.to_radians());
    assert!(a10.x_cp < a0.x_cp && a10.x_cp > 0.5);
    assert!(a10.cn > a0.cn_alpha * 10.0_f64.to_radians().sin());
    assert_eq!(a0.cn, 0.0);
}

/// Compressibilidade: CNα das aletas cresce até o transônico e cai no supersônico,
/// sem saltos.
#[test]
fn fin_lift_slope_vs_mach_is_continuous() {
    let f = FinSet {
        count: 3,
        planform: FinPlanform::Trapezoid {
            root_chord: 0.2,
            tip_chord: 0.1,
            sweep: 0.1,
        },
        span: 0.12,
        thickness: 0.004,
        position: 0.0,
        profile: FinProfile::Rounded,
    };
    let a = 0.01;
    assert!(f.single_fin_cn_alpha(0.8, a) > f.single_fin_cn_alpha(0.0, a));
    assert!(f.single_fin_cn_alpha(3.0, a) < f.single_fin_cn_alpha(1.5, a));
    let mut prev = f.single_fin_cn_alpha(0.0, a);
    for i in 1..=400 {
        let v = f.single_fin_cn_alpha(f64::from(i) * 0.01, a);
        assert!(
            (v / prev - 1.0).abs() < 0.03,
            "salto em M = {}",
            f64::from(i) * 0.01
        );
        prev = v;
    }
}

fn model_rocket() -> Geometry {
    Geometry {
        parts: vec![
            BodyPart::Nose {
                shape: NoseShape::TangentOgive,
                length: 0.1,
                diameter: 0.025,
            },
            BodyPart::Tube {
                length: 0.35,
                diameter: 0.025,
            },
        ],
        fins: vec![FinSet {
            count: 3,
            planform: FinPlanform::Trapezoid {
                root_chord: 0.05,
                tip_chord: 0.03,
                sweep: 0.02,
            },
            span: 0.04,
            thickness: 0.002,
            position: 0.40,
            profile: FinProfile::Rounded,
        }],
        roughness: 6e-5,
        nozzle_exit_diameter: 0.0,
    }
}

/// Parcelas do arrasto contra valores calculados à mão.
#[test]
fn drag_components_match_hand_calculation() {
    let g = model_rocket();
    // Corpo liso, Re = 1e6, incompressível: Cf = 1/(1,5·ln 1e6 − 5,6)² = 0,004372.
    let mut smooth = g.clone();
    smooth.roughness = 0.0;
    let re_per_m = 1e6 / smooth.length();
    let d = smooth.drag(&FlowState {
        mach: 0.0,
        reynolds_per_m: re_per_m,
        burning: false,
    });
    let cf = 1.0 / (1.5 * 1e6_f64.ln() - 5.6).powi(2);
    assert!((cf - 0.004_372).abs() < 1e-6);
    let wet_tube = PI * 0.025 * 0.35;
    let fins = (1.0 + 2.0 * 0.002 / (2.0 / 3.0 * (0.08 - 0.0015 / 0.08))) * 2.0 * 3.0 * 0.04 * 0.04;
    let a_ref = PI * 0.025_f64.powi(2) / 4.0;
    // Área molhada da ogiva: entre a do cone e a do cilindro de mesmas dimensões.
    let lo = cf * ((1.0 + 0.025 / 0.9) * (wet_tube + PI * 0.0125 * 0.1) + fins) / a_ref;
    let hi = cf * ((1.0 + 0.025 / 0.9) * (wet_tube + PI * 0.025 * 0.1) + fins) / a_ref;
    assert!(
        d.friction > lo && d.friction < hi,
        "{} fora de [{lo}, {hi}]",
        d.friction
    );
    // Base: 0,12 em M = 0; ogiva tangente não tem arrasto de pressão no subsônico.
    assert!((d.base - 0.12).abs() < 1e-12);
    assert_eq!(d.forebody, 0.0);
    // Aletas: bordo arredondado tem arrasto nulo em M = 0; sobra metade do de base.
    let frontal = 3.0 * 0.002 * 0.04 / a_ref;
    assert!((d.fins - frontal * 0.06).abs() < 1e-12);
}

#[test]
fn drag_is_plausible_and_continuous_in_mach() {
    let g = model_rocket();
    let cd = |m: f64, burning| {
        // Nível do mar: ν = 1,46e-5 m²/s.
        g.drag(&FlowState {
            mach: m,
            reynolds_per_m: m.max(0.01) * 340.3 / 1.46e-5,
            burning,
        })
        .total()
    };
    let low = cd(0.2, false);
    assert!(
        low > 0.3 && low < 0.9,
        "Cd subsônico de modelo pequeno: {low}"
    );
    assert!(cd(1.1, false) > cd(0.6, false), "pico transônico");
    assert!(cd(3.0, false) < cd(1.2, false), "queda no supersônico");
    let mut prev = cd(0.05, false);
    for i in 6..=400 {
        let v = cd(f64::from(i) * 0.01, false);
        assert!(
            (v / prev - 1.0).abs() < 0.06,
            "salto em M = {}: {prev} → {v}",
            f64::from(i) * 0.01
        );
        prev = v;
    }
    // Rugosidade aumenta o atrito; motor aceso reduz o arrasto de base.
    let mut rough = g.clone();
    rough.roughness = 5e-4;
    let f = FlowState {
        mach: 0.3,
        reynolds_per_m: 7e6,
        burning: false,
    };
    assert!(rough.drag(&f).friction > g.drag(&f).friction);
    let mut with_nozzle = g.clone();
    with_nozzle.nozzle_exit_diameter = 0.018;
    assert!(with_nozzle.drag(&FlowState { burning: true, ..f }).base < with_nozzle.drag(&f).base);
}

/// Cone no supersônico: 2,1·sin²ε + 0,5·sinε/√(M² − 1). Boattail longo zera a parcela
/// de base da área removida.
#[test]
fn cone_wave_drag_and_boattail() {
    let g = nose_only(NoseShape::Conical, 0.3, 0.1);
    let f = FlowState {
        mach: 2.0,
        reynolds_per_m: 4e7,
        burning: false,
    };
    let s = 0.05 / f64::hypot(0.05, 0.3);
    assert!((g.drag(&f).forebody - (2.1 * s * s + 0.5 * s / 3.0_f64.sqrt())).abs() < 1e-12);

    let tail = |len: f64| {
        let mut b = nose_only(NoseShape::Conical, 0.3, 0.1);
        b.parts.push(BodyPart::Transition {
            length: len,
            d_fore: 0.1,
            d_aft: 0.06,
        });
        b.drag(&FlowState {
            mach: 0.5,
            reynolds_per_m: 1e7,
            burning: false,
        })
        .base
    };
    let cd_b = 0.12 + 0.13 * 0.25;
    assert!(
        (tail(0.2) - cd_b * 0.36).abs() < 1e-12,
        "boattail longo: só a base final"
    );
    assert!(
        (tail(0.01) - cd_b).abs() < 1e-12,
        "boattail curto: como base cheia"
    );
}

/// Arrasto por peça soma o total; a base fica no último trecho; um tubo sem nariz
/// ganha arrasto de frente rombuda.
#[test]
fn drag_by_part_sums_to_total_and_blunt_front() {
    let g = model_rocket();
    let aero = Aero::new(g.clone());
    let f = FlowState {
        mach: 0.4,
        reynolds_per_m: 8e6,
        burning: false,
    };
    let (total, parts) = aero.drag_parts(&f);
    let sum: f64 = parts.body.iter().chain(&parts.fins).sum();
    assert!((sum - total.total()).abs() < 1e-12);
    assert!((total.total() - g.drag(&f).total()).abs() < 1e-15);
    assert!(parts.body[1] > total.base, "o tubo traseiro leva a base");
    let mut headless = g.clone();
    headless.parts.remove(0);
    let blunt = headless.drag(&f);
    assert!(blunt.forebody > 0.8, "face plana: {}", blunt.forebody);
    assert!(blunt.total() > 2.0 * g.drag(&f).total());
}
