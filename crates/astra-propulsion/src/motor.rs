/// Ponto da tabela de um motor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotorPoint {
    /// Tempo desde a ignição, s.
    pub t: f64,
    /// Empuxo na pressão ambiente de referência, N.
    pub thrust: f64,
    /// Massa de propelente restante, kg.
    pub prop_mass: f64,
}

/// Motor como tabela de empuxo e massa de propelente no tempo (interpolação linear).
#[derive(Clone, Debug, PartialEq)]
pub struct Motor {
    /// Nome ou designação.
    pub name: String,
    /// Fabricante ou origem.
    pub manufacturer: String,
    /// Diâmetro externo, m.
    pub diameter: f64,
    /// Comprimento, m.
    pub length: f64,
    /// Massa sem propelente (invólucro, tubeira), kg.
    pub dry_mass: f64,
    /// Tabela em ordem crescente de tempo; o primeiro ponto é t = 0.
    pub points: Vec<MotorPoint>,
    /// Pressão ambiente em que o empuxo da tabela vale, Pa.
    pub ref_pressure: f64,
    /// Área de saída da tubeira, m² (0 se desconhecida: sem correção de altitude).
    pub exit_area: f64,
}

/// Pressão ao nível do mar padrão, Pa.
pub(crate) const P_SL: f64 = 101_325.0;

impl Motor {
    /// Ausência de motor: tabela sem empuxo e sem propelente.
    #[must_use]
    pub fn none() -> Self {
        let z = |t| MotorPoint {
            t,
            thrust: 0.0,
            prop_mass: 0.0,
        };
        Self {
            name: "sem motor".to_owned(),
            manufacturer: String::new(),
            diameter: 0.0,
            length: 0.0,
            dry_mass: 0.0,
            points: vec![z(0.0), z(1e-6)],
            ref_pressure: P_SL,
            exit_area: 0.0,
        }
    }

    /// Monta um motor a partir de pares (tempo, empuxo), assumindo impulso específico
    /// constante: a massa de propelente cai na proporção do impulso já entregue,
    /// m(t) = m_p·(1 − I(t)/I_total). Se o primeiro ponto tem t > 0, um ponto (0, 0)
    /// é acrescentado (convenção do formato RASP).
    #[must_use]
    pub fn from_thrust_curve(
        name: &str,
        manufacturer: &str,
        diameter: f64,
        length: f64,
        prop_mass: f64,
        dry_mass: f64,
        curve: &[(f64, f64)],
    ) -> Self {
        let mut pts: Vec<(f64, f64)> = Vec::with_capacity(curve.len() + 1);
        if curve.first().is_some_and(|p| p.0 > 0.0) {
            pts.push((0.0, 0.0));
        }
        pts.extend_from_slice(curve);
        let mut cum = vec![0.0; pts.len()];
        for i in 1..pts.len() {
            cum[i] = cum[i - 1] + 0.5 * (pts[i].1 + pts[i - 1].1) * (pts[i].0 - pts[i - 1].0);
        }
        let total = cum.last().copied().unwrap_or(0.0);
        let points = pts
            .iter()
            .zip(&cum)
            .map(|(&(t, thrust), &i)| MotorPoint {
                t,
                thrust,
                prop_mass: if total > 0.0 {
                    prop_mass * (1.0 - i / total)
                } else {
                    0.0
                },
            })
            .collect();
        Self {
            name: name.to_owned(),
            manufacturer: manufacturer.to_owned(),
            diameter,
            length,
            dry_mass,
            points,
            ref_pressure: P_SL,
            exit_area: 0.0,
        }
    }

    fn segment(&self, t: f64) -> Option<(&MotorPoint, &MotorPoint)> {
        if t < 0.0 || t > self.burn_time() {
            return None;
        }
        let i = self
            .points
            .partition_point(|p| p.t <= t)
            .clamp(1, self.points.len() - 1);
        Some((&self.points[i - 1], &self.points[i]))
    }

    /// O mesmo motor com a ignição atrasada de `t0` segundos: empuxo nulo e propelente
    /// cheio até `t0`, depois a curva original deslocada.
    #[must_use]
    pub fn delayed(&self, t0: f64) -> Self {
        if t0 <= 0.0 || !t0.is_finite() {
            return self.clone();
        }
        let full = self.initial_prop_mass();
        let mut points = vec![MotorPoint {
            t: 0.0,
            thrust: 0.0,
            prop_mass: full,
        }];
        points.extend(self.points.iter().map(|p| MotorPoint { t: p.t + t0, ..*p }));
        Self {
            points,
            ..self.clone()
        }
    }

    /// Instante do último ponto da tabela, s.
    #[must_use]
    pub fn burn_time(&self) -> f64 {
        self.points.last().map_or(0.0, |p| p.t)
    }

    /// Empuxo na pressão de referência, N.
    #[must_use]
    pub fn thrust_ref(&self, t: f64) -> f64 {
        self.segment(t).map_or(0.0, |(a, b)| {
            a.thrust + (b.thrust - a.thrust) * (t - a.t) / (b.t - a.t)
        })
    }

    /// Empuxo sob pressão ambiente `p_amb`: F = F_ref + (p_ref − p_amb)·A_e, enquanto o
    /// motor produz empuxo.
    /// Fonte: Sutton & Biblarz, "Rocket Propulsion Elements", 9ª ed., cap. 2 (equação do empuxo).
    #[must_use]
    pub fn thrust(&self, t: f64, p_amb: f64) -> f64 {
        let f = self.thrust_ref(t);
        if f > 0.0 {
            (f + (self.ref_pressure - p_amb) * self.exit_area).max(0.0)
        } else {
            0.0
        }
    }

    /// Massa de propelente restante, kg.
    #[must_use]
    pub fn prop_mass(&self, t: f64) -> f64 {
        if t <= 0.0 {
            return self.points.first().map_or(0.0, |p| p.prop_mass);
        }
        self.segment(t)
            .map_or(self.points.last().map_or(0.0, |p| p.prop_mass), |(a, b)| {
                a.prop_mass + (b.prop_mass - a.prop_mass) * (t - a.t) / (b.t - a.t)
            })
    }

    /// Vazão mássica, kg/s (constante em cada trecho da tabela, de modo que a massa
    /// integrada coincide com a tabela em todos os pontos).
    #[must_use]
    pub fn mdot(&self, t: f64) -> f64 {
        self.segment(t)
            .map_or(0.0, |(a, b)| (a.prop_mass - b.prop_mass) / (b.t - a.t))
    }

    /// Impulso total na pressão de referência, N·s (regra dos trapézios).
    #[must_use]
    pub fn total_impulse(&self) -> f64 {
        self.points
            .windows(2)
            .map(|w| 0.5 * (w[0].thrust + w[1].thrust) * (w[1].t - w[0].t))
            .sum()
    }

    /// Empuxo máximo da tabela, N.
    #[must_use]
    pub fn peak_thrust(&self) -> f64 {
        self.points.iter().map(|p| p.thrust).fold(0.0, f64::max)
    }

    /// Empuxo médio, N.
    #[must_use]
    pub fn average_thrust(&self) -> f64 {
        if self.burn_time() > 0.0 {
            self.total_impulse() / self.burn_time()
        } else {
            0.0
        }
    }

    /// Massa inicial de propelente, kg.
    #[must_use]
    pub fn initial_prop_mass(&self) -> f64 {
        self.points.first().map_or(0.0, |p| p.prop_mass)
    }

    /// Impulso específico médio, s: I_total/(m_p·g0).
    #[must_use]
    pub fn specific_impulse(&self) -> f64 {
        let mp = self.initial_prop_mass() - self.points.last().map_or(0.0, |p| p.prop_mass);
        if mp > 0.0 {
            self.total_impulse() / (mp * 9.806_65)
        } else {
            0.0
        }
    }

    /// Letra da classe de impulso: A = até 2,5 N·s, dobrando a cada letra
    /// (NFPA 1125 / NAR). Devolve `None` abaixo de 0,3125 N·s (classes fracionárias) ou
    /// acima da classe Z.
    #[must_use]
    pub fn impulse_class(&self) -> Option<char> {
        let i = self.total_impulse();
        if i <= 1.25 {
            return None;
        }
        let idx = (i / 2.5).log2().ceil().max(0.0) as u32;
        char::from_u32('A' as u32 + idx).filter(char::is_ascii_uppercase)
    }

    /// Instantes da tabela, s (quinas da curva de empuxo).
    #[must_use]
    pub fn breakpoints(&self) -> Vec<f64> {
        self.points.iter().map(|p| p.t).collect()
    }

    /// Exporta no formato RASP (.eng).
    #[must_use]
    pub fn to_eng(&self) -> String {
        let mp = self.initial_prop_mass();
        let mut s = format!(
            "; exportado pelo ASTRA\n{} {:.1} {:.1} 0 {:.4} {:.4} {}\n",
            self.name.replace(' ', "_"),
            self.diameter * 1000.0,
            self.length * 1000.0,
            mp,
            mp + self.dry_mass,
            if self.manufacturer.is_empty() {
                "ASTRA"
            } else {
                &self.manufacturer
            }
            .replace(' ', "_"),
        );
        for p in self.points.iter().filter(|p| p.t > 0.0) {
            s.push_str(&format!("{:.4} {:.3}\n", p.t, p.thrust));
        }
        s.push_str(";\n");
        s
    }
}

/// Lê um arquivo RASP (.eng), que pode conter vários motores.
///
/// Formato: linhas de comentário começam com `;`. Cabeçalho:
/// `nome diâmetro_mm comprimento_mm atrasos massa_propelente_kg massa_total_kg fabricante`.
/// Depois, pares `tempo_s empuxo_N` até um ponto de empuxo zero.
/// Fonte: especificação do formato em thrustcurve.org ("RASP engine file format").
///
/// # Errors
/// Mensagem com o número da linha quando o texto não segue o formato.
pub fn parse_eng(text: &str) -> Result<Vec<Motor>, String> {
    let mut motors = Vec::new();
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty() && !l.starts_with(';'));
    while let Some((n, header)) = lines.next() {
        let f: Vec<&str> = header.split_whitespace().collect();
        if f.len() < 7 {
            return Err(format!(
                "linha {n}: cabeçalho de motor com menos de 7 campos"
            ));
        }
        let num = |i: usize, what: &str| {
            f[i].parse::<f64>()
                .map_err(|_| format!("linha {n}: {what} inválido: \"{}\"", f[i]))
        };
        let (d, l, mp, mt) = (
            num(1, "diâmetro")?,
            num(2, "comprimento")?,
            num(4, "massa de propelente")?,
            num(5, "massa total")?,
        );
        let mut curve = Vec::new();
        for (n, line) in lines.by_ref() {
            let mut it = line.split_whitespace();
            let (Some(a), Some(b)) = (it.next(), it.next()) else {
                return Err(format!("linha {n}: esperado \"tempo empuxo\""));
            };
            let (t, thrust) = (
                a.parse::<f64>()
                    .map_err(|_| format!("linha {n}: tempo inválido"))?,
                b.parse::<f64>()
                    .map_err(|_| format!("linha {n}: empuxo inválido"))?,
            );
            if curve.last().is_some_and(|p: &(f64, f64)| t <= p.0) {
                return Err(format!("linha {n}: tempos fora de ordem"));
            }
            curve.push((t, thrust));
            if thrust == 0.0 {
                break;
            }
        }
        if curve.len() < 2 {
            return Err(format!("linha {n}: motor \"{}\" sem curva de empuxo", f[0]));
        }
        motors.push(Motor::from_thrust_curve(
            f[0],
            f[6],
            d / 1000.0,
            l / 1000.0,
            mp,
            mt - mp,
            &curve,
        ));
    }
    if motors.is_empty() {
        return Err("nenhum motor encontrado no arquivo".to_owned());
    }
    Ok(motors)
}

fn xml_attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let mut rest = tag;
    while let Some(i) = rest.find(name) {
        let before_ok = rest[..i]
            .chars()
            .next_back()
            .is_none_or(char::is_whitespace);
        let after = rest[i + name.len()..].trim_start();
        if before_ok && after.starts_with('=') {
            let v = after[1..].trim_start();
            let quote = v.chars().next()?;
            let v = &v[1..];
            return v.find(quote).map(|end| &v[..end]);
        }
        rest = &rest[i + name.len()..];
    }
    None
}

/// Lê um arquivo RockSim (.rse). Usa os atributos `code`, `mfg`, `dia` e `len` (mm),
/// `propWt` e `initWt` (g) de `<engine>` e `t` (s), `f` (N) e `m` (g, massa de
/// propelente restante) de cada `<eng-data>`. Sem `m`, a massa segue o impulso.
///
/// # Errors
/// Mensagem quando falta um atributo obrigatório ou não há motores.
pub fn parse_rse(text: &str) -> Result<Vec<Motor>, String> {
    let mut motors = Vec::new();
    for chunk in text.split("<engine ").skip(1) {
        let head = chunk.split('>').next().unwrap_or("");
        let num = |name: &str| -> Result<f64, String> {
            xml_attr(head, name)
                .and_then(|v| v.trim().parse::<f64>().ok())
                .ok_or_else(|| format!("atributo \"{name}\" ausente ou inválido em <engine>"))
        };
        let (dia, len, prop, init) = (num("dia")?, num("len")?, num("propWt")?, num("initWt")?);
        let body = chunk.split("</engine>").next().unwrap_or("");
        let mut curve = Vec::new();
        let mut masses = Vec::new();
        for d in body.split("<eng-data").skip(1) {
            let tag = d.split('>').next().unwrap_or("");
            let get = |n: &str| xml_attr(tag, n).and_then(|v| v.trim().parse::<f64>().ok());
            let (Some(t), Some(f)) = (get("t"), get("f")) else {
                return Err("ponto <eng-data> sem \"t\" ou \"f\"".to_owned());
            };
            curve.push((t, f));
            masses.push(get("m"));
        }
        if curve.len() < 2 {
            return Err("motor sem pontos <eng-data>".to_owned());
        }
        let mut m = Motor::from_thrust_curve(
            xml_attr(head, "code").unwrap_or("motor"),
            xml_attr(head, "mfg").unwrap_or(""),
            dia / 1000.0,
            len / 1000.0,
            prop / 1000.0,
            (init - prop) / 1000.0,
            &curve,
        );
        if masses.iter().all(Option::is_some) && m.points.len() == masses.len() {
            for (p, g) in m.points.iter_mut().zip(&masses) {
                p.prop_mass = g.unwrap_or(0.0) / 1000.0;
            }
        }
        motors.push(m);
    }
    if motors.is_empty() {
        return Err("nenhum <engine> encontrado no arquivo".to_owned());
    }
    Ok(motors)
}
