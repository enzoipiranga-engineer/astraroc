//! Vetoração de empuxo (TVC): motor com gimbal e controle de atitude.

use crate::Rocket;
use astra_dynamics::flight::{FlightCtx, Loads, MassProps, RigidVehicle, Sequencer, Trigger};
use astra_math::Vec3;
use core::f64::consts::PI;

/// Giroscópio simulado: viés constante mais ruído de banda limitada (soma de senoides
/// de fase aleatória, contínua no tempo para o integrador adaptativo).
#[derive(Clone, Debug, PartialEq)]
pub struct Gyro {
    /// Viés, rad/s, por eixo (y, z do corpo).
    pub bias: (f64, f64),
    /// Desvio-padrão do ruído, rad/s.
    pub noise: f64,
    /// Semente.
    pub seed: u64,
}

impl Gyro {
    /// Giroscópio perfeito.
    #[must_use]
    pub fn ideal() -> Self {
        Self {
            bias: (0.0, 0.0),
            noise: 0.0,
            seed: 0,
        }
    }

    fn noise_at(&self, t: f64, axis: u64) -> f64 {
        if self.noise == 0.0 {
            return 0.0;
        }
        // 16 senoides entre 1 e 40 Hz com fases de SplitMix64; variância total = σ².
        let mut s = self.seed ^ (axis.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let mut next = || {
            s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = s;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
        };
        let n = 16;
        let amp = self.noise * (2.0 / f64::from(n)).sqrt();
        (0..n)
            .map(|i| {
                let f = 1.0 * 40.0_f64.powf(f64::from(i) / f64::from(n - 1));
                amp * (2.0 * PI * f * t + 2.0 * PI * next()).sin()
            })
            .sum()
    }

    /// Leitura (y, z) da velocidade angular transversal verdadeira `w` no instante `t`.
    #[must_use]
    pub fn read(&self, t: f64, w: Vec3) -> (f64, f64) {
        (
            w.y + self.bias.0 + self.noise_at(t, 1),
            w.z + self.bias.1 + self.noise_at(t, 2),
        )
    }
}

/// Controlador PD de atitude que comanda o gimbal.
#[derive(Clone, Debug, PartialEq)]
pub struct Tvc {
    /// Ângulo máximo de gimbal, rad.
    pub max_gimbal: f64,
    /// Frequência natural desejada da malha de atitude, rad/s.
    pub omega_n: f64,
    /// Amortecimento desejado.
    pub zeta: f64,
    /// Direção desejada do eixo do foguete: a vertical local, inclinada de `tilt` rad
    /// para o azimute `azimuth` (rad a partir do norte), em torno do ponto atual.
    pub tilt: f64,
    /// Azimute da inclinação desejada, rad.
    pub azimuth: f64,
    /// Sensor.
    pub gyro: Gyro,
}

/// Foguete com TVC.
#[derive(Clone, Debug)]
pub struct TvcRocket {
    /// Foguete (aerodinâmica, massa, motor).
    pub rocket: Rocket,
    /// Controle.
    pub tvc: Tvc,
}

impl TvcRocket {
    /// Ângulos de gimbal (δy, δz) para o estado dado.
    ///
    /// Lei de controle PD por alocação de polos: o momento desejado em torno do CG é
    /// M = I_t·(ω_n²·e − 2ζω_n·ω_medido), com e o vetor de erro de atitude (eixo do corpo
    /// × direção desejada, nos eixos do corpo). O momento de um empuxo T defletido de δ
    /// na tubeira, a uma distância l atrás do CG, é M_y = l·T·δz e M_z = −l·T·δy (ângulos
    /// pequenos). A deflexão é saturada em `max_gimbal`.
    /// Fonte da lei: Ogata, "Modern Control Engineering", 5ª ed., cap. 5 (sistema de 2ª
    /// ordem); Wie, "Space Vehicle Dynamics and Control", 2ª ed., cap. 6 (TVC).
    #[must_use]
    pub fn gimbal(&self, t: f64, thrust: f64, target_body: Vec3, w: Vec3) -> (f64, f64) {
        if thrust <= 0.0 {
            return (0.0, 0.0);
        }
        let it = self.rocket.inertia(t).1;
        let arm = (self.rocket.nozzle_station - self.rocket.cg(t)).max(1e-3);
        let e = Vec3::X.cross(target_body);
        let (gy, gz) = self.tvc.gyro.read(t, w);
        let wn = self.tvc.omega_n;
        let my = it * (wn * wn * e.y - 2.0 * self.tvc.zeta * wn * gy);
        let mz = it * (wn * wn * e.z - 2.0 * self.tvc.zeta * wn * gz);
        let mut dz = my / (arm * thrust);
        let mut dy = -mz / (arm * thrust);
        let mag = dy.hypot(dz);
        if mag > self.tvc.max_gimbal {
            dy *= self.tvc.max_gimbal / mag;
            dz *= self.tvc.max_gimbal / mag;
        }
        (dy, dz)
    }
}

impl Sequencer for TvcRocket {
    fn burnout_time(&self) -> f64 {
        self.rocket.burnout_time()
    }
    fn breakpoints(&self) -> Vec<f64> {
        self.rocket.breakpoints()
    }
    fn triggers(&self) -> Vec<Trigger> {
        self.rocket.triggers()
    }
}

impl RigidVehicle for TvcRocket {
    fn loads(&self, c: &FlightCtx<'_>, v: Vec3, w: Vec3) -> Loads {
        let mut l = self.rocket.loads(c, v, w);
        if !c.burning || c.on_rail {
            return l;
        }
        let thrust = self.rocket.motor.thrust(c.t, c.atmo.pressure);
        let Some(q) = c.attitude else { return l };
        // Alvo: vertical local inclinada de `tilt` para o azimute pedido.
        let up = c.up;
        let east = Vec3::Z.cross(up).normalized().unwrap_or(Vec3::X);
        let north = up.cross(east);
        let horiz = east * self.tvc.azimuth.sin() + north * self.tvc.azimuth.cos();
        let target = up * self.tvc.tilt.cos() + horiz * self.tvc.tilt.sin();
        let target_body = q.conj().rotate(target);
        let (dy, dz) = self.gimbal(c.t, thrust, target_body, w);
        let arm = self.rocket.nozzle_station - self.rocket.cg(c.t);
        // Troca o empuxo axial pelo defletido e soma o momento.
        let dir = Vec3::new((1.0 - dy * dy - dz * dz).max(0.0).sqrt(), dy, dz);
        l.force_body = l.force_body - Vec3::new(thrust, 0.0, 0.0) + dir * thrust;
        l.moment_body += Vec3::new(0.0, arm * thrust * dir.z, -arm * thrust * dir.y);
        l
    }
    fn mass_props(&self, t: f64, mass: f64) -> MassProps {
        self.rocket.mass_props(t, mass)
    }
}
