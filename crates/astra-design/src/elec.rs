//! Eletrônica embarcada: componentes, fiação, verificações elétricas e caminhos
//! pirotécnicos.

use crate::checks::{Issue, Severity};
use crate::model::{Design, PartKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Pino de um componente.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pin {
    /// Nome.
    pub name: String,
    /// Tipo: vin, vout, vcc, gnd, io, sda, scl, tx, rx, mosi, miso, sck, cs, sink, bat+,
    /// bat-, gate, drain, source, pyro, switch.
    pub kind: String,
    /// Tensão (saídas de regulador), V.
    #[serde(default)]
    pub volts: Option<f64>,
    /// Corrente máxima (saídas de regulador), mA.
    #[serde(default)]
    pub max_ma: Option<f64>,
}

/// Faixa de alimentação, V.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Supply {
    /// Mínimo.
    pub min: f64,
    /// Máximo.
    pub max: f64,
}

/// Bateria.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Battery {
    /// Tensão nominal, V.
    pub volts: f64,
    /// Capacidade, mAh.
    pub capacity_mah: f64,
    /// Resistência interna, Ω.
    pub internal_ohm: f64,
}

/// MOSFET.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mosfet {
    /// Tensão de gate para conduzir plenamente, V.
    pub vgs_on: f64,
    /// Resistência ligada, Ω.
    pub rds_on: f64,
    /// Corrente máxima, A.
    pub max_a: f64,
}

/// E-match.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ematch {
    /// Resistência, Ω.
    pub resistance_ohm: f64,
    /// Corrente que garante o disparo, A.
    pub all_fire_a: f64,
    /// Corrente que garantidamente não dispara, A.
    pub no_fire_a: f64,
}

/// Rádio.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Radio {
    /// Potência de transmissão, dBm.
    pub tx_dbm: f64,
    /// Sensibilidade, dBm.
    pub sensitivity_dbm: f64,
    /// Frequência, MHz.
    pub freq_mhz: f64,
    /// Ganho da antena, dBi.
    pub antenna_dbi: f64,
}

/// Componente da biblioteca.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElecComponent {
    /// Identificador.
    pub id: String,
    /// Nome.
    pub name: String,
    /// Categoria.
    pub category: String,
    /// Massa, g.
    pub mass_g: f64,
    /// Comprimento, mm.
    pub length_mm: f64,
    /// Largura, mm.
    pub width_mm: f64,
    /// Pinos.
    pub pins: Vec<Pin>,
    /// Alimentação.
    #[serde(default)]
    pub supply: Option<Supply>,
    /// Consumo típico, mA.
    #[serde(default)]
    pub current_ma: f64,
    /// Consumo de pico, mA.
    #[serde(default)]
    pub peak_ma: f64,
    /// Corrente drenada de um pino de saída (cargas), mA.
    #[serde(default)]
    pub load_ma: f64,
    /// Nível lógico (placas), V.
    #[serde(default)]
    pub logic_v: Option<f64>,
    /// Corrente recomendada por pino (placas), mA.
    #[serde(default)]
    pub pin_max_ma: Option<f64>,
    /// Corrente máxima absoluta por pino, mA.
    #[serde(default)]
    pub pin_abs_max_ma: Option<f64>,
    /// Tensão de brownout do microcontrolador, V.
    #[serde(default)]
    pub brownout_v: Option<f64>,
    /// Microcontrolador (para a emulação).
    #[serde(default)]
    pub mcu: Option<String>,
    /// Endereços I2C possíveis.
    #[serde(default)]
    pub i2c: Vec<u8>,
    /// Características do sensor (usadas pela simulação dos sensores no navegador).
    #[serde(default)]
    pub sensor: Option<serde_json::Value>,
    /// Tem antena.
    #[serde(default)]
    pub antenna: bool,
    /// Rádio.
    #[serde(default)]
    pub radio: Option<Radio>,
    /// Bateria.
    #[serde(default)]
    pub battery: Option<Battery>,
    /// MOSFET.
    #[serde(default)]
    pub mosfet: Option<Mosfet>,
    /// E-match.
    #[serde(default)]
    pub ematch: Option<Ematch>,
    /// Fonte dos dados.
    pub source: String,
    /// Observações.
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Deserialize)]
struct LibFile {
    components: Vec<ElecComponent>,
}

/// Biblioteca embutida (`data/electronics.json`).
///
/// # Panics
/// Nunca: o arquivo embutido é válido (coberto por teste).
#[must_use]
pub fn library() -> Vec<ElecComponent> {
    serde_json::from_str::<LibFile>(include_str!("../../../data/electronics.json"))
        .expect("biblioteca de eletrônica embutida")
        .components
}

/// Componente instalado no foguete.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EComp {
    /// Identificador no projeto.
    pub id: String,
    /// Componente da biblioteca.
    pub lib: String,
    /// Peça física (tipo massa) que o representa no foguete.
    pub part: String,
    /// Endereço I2C escolhido.
    #[serde(default)]
    pub i2c_address: Option<u8>,
    /// Carga da bateria, 0 a 1.
    #[serde(default)]
    pub charge: Option<f64>,
}

/// Fio entre dois pinos ("componente:pino").
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Wire {
    /// Uma ponta.
    pub a: String,
    /// Outra ponta.
    pub b: String,
    /// Fio mal crimpado: solta quando a aceleração passa deste valor, g.
    #[serde(default)]
    pub loose_g: Option<f64>,
}

/// Firmware carregado numa placa.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Firmware {
    /// Placa (componente).
    pub board: String,
    /// Nome do arquivo.
    pub name: String,
    /// Conteúdo Intel HEX.
    pub hex: String,
}

/// Disparo de um canal pirotécnico.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PyroEvent {
    /// E-match.
    pub ematch: String,
    /// Instante, s.
    pub t: f64,
}

/// Eletrônica do projeto.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Electronics {
    /// Componentes.
    #[serde(default)]
    pub components: Vec<EComp>,
    /// Fios.
    #[serde(default)]
    pub wires: Vec<Wire>,
    /// Firmware.
    #[serde(default)]
    pub firmware: Option<Firmware>,
    /// Tempo de espera na rampa com tudo ligado, h.
    #[serde(default = "default_standby")]
    pub standby_hours: f64,
    /// Disparos pirotécnicos produzidos pelo firmware (preenchido pela emulação).
    #[serde(default)]
    pub pyro_events: Vec<PyroEvent>,
}

fn default_standby() -> f64 {
    1.0
}

/// Caminho de disparo de um e-match.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PyroPath {
    /// E-match.
    pub ematch: String,
    /// Placa que comanda.
    pub board: Option<String>,
    /// Pino da placa.
    pub pin: Option<String>,
    /// O disparo acende o e-match.
    pub fires: bool,
    /// Corrente no disparo, A.
    pub current: f64,
    /// A placa reinicia (brownout) no disparo.
    pub brownout: bool,
    /// Explicação.
    pub reason: String,
}

/// Dispositivo para a emulação.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// Componente.
    pub comp: String,
    /// Biblioteca.
    pub lib: String,
    /// Endereço I2C.
    pub address: Option<u8>,
    /// Está no barramento I2C da placa com firmware.
    pub on_bus: bool,
    /// Alimentado corretamente.
    pub powered: bool,
    /// Antena bloqueada pela fuselagem.
    pub blocked: bool,
}

/// Resultado da análise elétrica.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElecReport {
    /// Problemas.
    pub issues: Vec<Issue>,
    /// Caminhos pirotécnicos.
    pub pyro: Vec<PyroPath>,
    /// Dispositivos.
    pub devices: Vec<Device>,
    /// Autonomia da bateria principal, h.
    pub autonomy_h: f64,
    /// Corrente total em espera, mA.
    pub standby_ma: f64,
    /// Alcance de rádio estimado, km.
    pub radio_range_km: Option<f64>,
    /// Placa com firmware e seu nível lógico.
    pub board: Option<String>,
}

struct Nets {
    parent: BTreeMap<String, String>,
}

impl Nets {
    fn find(&mut self, x: &str) -> String {
        let p = self.parent.get(x).cloned().unwrap_or_else(|| x.to_owned());
        if p == x {
            self.parent.insert(x.to_owned(), x.to_owned());
            return p;
        }
        let r = self.find(&p);
        self.parent.insert(x.to_owned(), r.clone());
        r
    }
    fn join(&mut self, a: &str, b: &str) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.parent.insert(ra, rb);
        }
    }
}

/// Atenuação de rádio pela fuselagem, dB, conforme o material do tubo.
fn attenuation(material: &str) -> f64 {
    if material.contains("cfrp")
        || material.starts_with("al-")
        || material.starts_with("aco")
        || material.starts_with("inox")
        || material.starts_with("ti-")
    {
        if material.contains("cfrp") {
            30.0
        } else {
            60.0
        }
    } else {
        1.0
    }
}

fn issue(
    sev: Severity,
    code: &str,
    message: String,
    suggestion: &str,
    parts: Vec<String>,
) -> Issue {
    Issue {
        severity: sev,
        code: code.into(),
        message,
        suggestion: suggestion.into(),
        parts,
        joints: vec![],
    }
}

/// Analisa a eletrônica do projeto.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn analyze(d: &Design, props: &[crate::PartProps]) -> ElecReport {
    let lib = library();
    let e = &d.electronics;
    let mut r = ElecReport::default();
    if e.components.is_empty() {
        return r;
    }
    let get = |c: &EComp| lib.iter().find(|l| l.id == c.lib);
    let mut nets = Nets {
        parent: BTreeMap::new(),
    };
    for w in &e.wires {
        nets.join(&w.a, &w.b);
    }
    for c in &e.components {
        if get(c).is_none() {
            r.issues.push(issue(
                Severity::Error,
                "unknownComponent",
                format!("Componente \"{}\" não existe na biblioteca.", c.lib),
                "Escolha um componente da lista.",
                vec![c.part.clone()],
            ));
        }
        if let Some(l) = get(c) {
            if l.category == "misc" && l.pins.iter().all(|p| p.kind == "switch") {
                // Chave de armar: fechada no voo.
                if let [a, b] = l.pins.as_slice() {
                    nets.join(
                        &format!("{}:{}", c.id, a.name),
                        &format!("{}:{}", c.id, b.name),
                    );
                }
            }
        }
    }
    let node = |c: &EComp, pin: &str| format!("{}:{}", c.id, pin);
    // Tensões das redes: baterias, depois reguladores das placas alimentadas.
    let mut volts: BTreeMap<String, f64> = BTreeMap::new();
    let mut ground: Option<String> = None;
    let mut batteries = Vec::new();
    for c in &e.components {
        let Some(l) = get(c) else { continue };
        if let Some(b) = l.battery {
            let plus = nets.find(&node(c, "+"));
            let minus = nets.find(&node(c, "-"));
            volts.insert(plus, b.volts);
            ground.get_or_insert(minus);
            batteries.push((c, b));
        }
    }
    let Some(gnd) = ground else {
        r.issues.push(issue(
            Severity::Error,
            "noBattery",
            "A eletrônica não tem bateria ligada.".into(),
            "Adicione uma bateria e ligue + e −.",
            vec![],
        ));
        return r;
    };
    let gnd_ok = |nets: &mut Nets, c: &EComp, l: &ElecComponent| {
        l.pins
            .iter()
            .filter(|p| p.kind == "gnd")
            .all(|p| nets.find(&node(c, &p.name)) == gnd)
    };
    let mut powered: BTreeMap<String, bool> = BTreeMap::new();
    for _ in 0..3 {
        for c in &e.components {
            let Some(l) = get(c) else { continue };
            if let Some(vin) = l.pins.iter().find(|p| p.kind == "vin") {
                let v = volts.get(&nets.find(&node(c, &vin.name))).copied();
                let on = v.is_some_and(|v| l.supply.is_none_or(|s| v >= s.min && v <= s.max))
                    && gnd_ok(&mut nets, c, l);
                powered.insert(c.id.clone(), on);
                if on {
                    for p in l.pins.iter().filter(|p| p.kind == "vout") {
                        let n = nets.find(&node(c, &p.name));
                        volts.entry(n).or_insert(p.volts.unwrap_or(5.0));
                    }
                }
            }
        }
    }
    // Placa com firmware.
    let board = e.firmware.as_ref().map(|f| f.board.clone()).or_else(|| {
        e.components
            .iter()
            .find(|c| get(c).is_some_and(|l| l.category == "placa"))
            .map(|c| c.id.clone())
    });
    r.board.clone_from(&board);
    let board_comp = board
        .as_ref()
        .and_then(|b| e.components.iter().find(|c| &c.id == b));
    let board_lib = board_comp.and_then(get);
    let logic = board_lib.and_then(|l| l.logic_v).unwrap_or(5.0);
    // Alimentação de cada componente.
    let mut load_on: BTreeMap<String, f64> = BTreeMap::new();
    let mut standby = 0.0;
    for c in &e.components {
        let Some(l) = get(c) else { continue };
        if l.battery.is_some() || l.category == "pirotecnia" || l.category == "misc" {
            continue;
        }
        let vin_pin = l.pins.iter().find(|p| p.kind == "vin" || p.kind == "vcc");
        let Some(vp) = vin_pin else { continue };
        let n = nets.find(&node(c, &vp.name));
        let v = volts.get(&n).copied();
        let part = vec![c.part.clone()];
        match (v, l.supply) {
            (None, _) => r.issues.push(issue(
                Severity::Error,
                "unpowered",
                format!("{} está sem alimentação no pino {}.", l.name, vp.name),
                "Ligue o pino de alimentação à bateria ou a uma saída de regulador.",
                part,
            )),
            (Some(v), Some(s)) if v > s.max => r.issues.push(issue(
                Severity::Error,
                "overvoltage",
                format!(
                    "{} recebe {v:.1} V, acima dos {:.1} V que aguenta: queima.",
                    l.name, s.max
                ),
                "Use a saída de 3,3 V ou um módulo com regulador.",
                part,
            )),
            (Some(v), Some(s)) if v < s.min => r.issues.push(issue(
                Severity::Error,
                "undervoltage",
                format!(
                    "{} recebe {v:.1} V, abaixo dos {:.1} V de que precisa: não liga.",
                    l.name, s.min
                ),
                "Use uma fonte de tensão maior.",
                part,
            )),
            _ => {}
        }
        if !gnd_ok(&mut nets, c, l) {
            r.issues.push(issue(
                Severity::Error,
                "noGround",
                format!(
                    "O GND de {} não está ligado ao negativo da bateria.",
                    l.name
                ),
                "Ligue todos os GND ao negativo da bateria.",
                vec![c.part.clone()],
            ));
        }
        *load_on.entry(n).or_default() += l.current_ma;
        standby += l.current_ma;
    }
    // Saídas de regulador sobrecarregadas.
    for c in &e.components {
        let Some(l) = get(c) else { continue };
        for p in l.pins.iter().filter(|p| p.kind == "vout") {
            let n = nets.find(&node(c, &p.name));
            if let (Some(load), Some(max)) = (load_on.get(&n), p.max_ma) {
                if *load > max {
                    r.issues.push(issue(Severity::Warning, "regulatorOverload", format!("A saída {} de {} alimenta {load:.0} mA, acima dos {max:.0} mA que fornece.", p.name, l.name), "Use um regulador separado.", vec![c.part.clone()]));
                }
            }
        }
    }
    // I2C: barramento da placa, endereços, nível lógico.
    let (sda, scl) = match (board_comp, board_lib) {
        (Some(bc), Some(bl)) => (
            bl.pins
                .iter()
                .find(|p| p.kind == "sda")
                .map(|p| nets.find(&node(bc, &p.name))),
            bl.pins
                .iter()
                .find(|p| p.kind == "scl")
                .map(|p| nets.find(&node(bc, &p.name))),
        ),
        _ => (None, None),
    };
    let mut used: BTreeMap<u8, String> = BTreeMap::new();
    for c in &e.components {
        let Some(l) = get(c) else { continue };
        let on_bus = !l.i2c.is_empty()
            && l.pins
                .iter()
                .any(|p| p.kind == "sda" && Some(nets.find(&node(c, &p.name))) == sda)
            && l.pins
                .iter()
                .any(|p| p.kind == "scl" && Some(nets.find(&node(c, &p.name))) == scl);
        let address = if l.i2c.is_empty() {
            None
        } else {
            Some(c.i2c_address.unwrap_or(l.i2c[0]))
        };
        let vcc_ok = !r
            .issues
            .iter()
            .any(|i| i.parts.contains(&c.part) && i.severity == Severity::Error);
        if let (Some(a), true) = (address, on_bus) {
            if let Some(other) = used.insert(a, l.name.clone()) {
                r.issues.push(issue(
                    Severity::Error,
                    "i2cConflict",
                    format!("{} e {other} usam o mesmo endereço I2C 0x{a:02X}.", l.name),
                    "Mude o endereço de um deles (pino SDO/AD0).",
                    vec![c.part.clone()],
                ));
            }
            if l.supply.is_some_and(|s| s.max < logic - 0.2) {
                r.issues.push(issue(
                    Severity::Warning,
                    "logicLevel",
                    format!(
                        "{} aguenta só {:.1} V, mas o I2C da placa trabalha em {logic:.1} V.",
                        l.name,
                        l.supply.map_or(0.0, |s| s.max)
                    ),
                    "Use um conversor de nível lógico ou módulo com regulador.",
                    vec![c.part.clone()],
                ));
            }
        } else if address.is_some() && board.is_some() {
            r.issues.push(issue(
                Severity::Warning,
                "notOnBus",
                format!("{} não está ligado ao SDA e ao SCL da placa.", l.name),
                "Ligue SDA com SDA e SCL com SCL.",
                vec![c.part.clone()],
            ));
        }
        // Antena dentro de fuselagem condutora.
        let mut blocked = false;
        if l.antenna {
            if let Some(pi) = d.parts.iter().position(|p| p.id == c.part) {
                let x = props.get(pi).map_or(0.0, |p| p.cg);
                if let Some((tube, _)) = d.parts.iter().zip(props).find(|(p, q)| {
                    matches!(
                        p.kind,
                        PartKind::BodyTube { .. } | PartKind::NoseCone { .. }
                    ) && q.x0 <= x
                        && x <= q.x1
                }) {
                    let att = attenuation(&tube.material);
                    if att >= 30.0 {
                        blocked = true;
                        r.issues.push(issue(Severity::Warning, "antennaBlocked", format!("A antena de {} está dentro de {} ({}): o sinal fica bloqueado (≈ {att:.0} dB).", l.name, tube.name, tube.material), "Ponha a antena numa seção de fibra de vidro, papel ou plástico.", vec![c.part.clone()]));
                    }
                    if let Some(rd) = l.radio {
                        // Orçamento de enlace: PL = 20·log10(d_km) + 20·log10(f_MHz) + 32,44 dB,
                        // com 10 dB de margem de desvanecimento e antena de 2,15 dBi em terra.
                        // Fonte: ITU-R P.525-4 (espaço livre).
                        let budget =
                            rd.tx_dbm + rd.antenna_dbi + 2.15 - rd.sensitivity_dbm - att - 10.0;
                        let km = 10f64.powf((budget - 20.0 * rd.freq_mhz.log10() - 32.44) / 20.0);
                        r.radio_range_km = Some(r.radio_range_km.map_or(km, |x: f64| x.max(km)));
                    }
                }
            }
        }
        r.devices.push(Device {
            comp: c.id.clone(),
            lib: l.id.clone(),
            address,
            on_bus,
            powered: vcc_ok,
            blocked,
        });
    }
    // Cargas em pinos de saída da placa.
    if let (Some(bc), Some(bl)) = (board_comp, board_lib) {
        let max = bl.pin_max_ma.unwrap_or(20.0);
        for p in bl.pins.iter().filter(|p| p.kind == "io") {
            let n = nets.find(&node(bc, &p.name));
            for c in &e.components {
                let Some(l) = get(c) else { continue };
                for q in l.pins.iter().filter(|q| q.kind == "sink") {
                    if nets.find(&node(c, &q.name)) == n && l.load_ma > max {
                        r.issues.push(issue(Severity::Warning, "pinCurrent", format!("O pino {} de {} alimenta {} ({:.0} mA), acima dos {max:.0} mA recomendados.", p.name, bl.name, l.name, l.load_ma), "Use um transistor ou MOSFET.", vec![c.part.clone()]));
                    }
                }
            }
        }
    }
    // Caminhos pirotécnicos.
    let main_batt = batteries.first().map(|(_, b)| *b);
    for c in &e.components {
        let Some(l) = get(c) else { continue };
        let Some(em) = l.ematch else { continue };
        let mut path = PyroPath {
            ematch: c.id.clone(),
            ..PyroPath::default()
        };
        let a = nets.find(&node(c, "A"));
        let b = nets.find(&node(c, "B"));
        // Ligado direto num pino da placa?
        let mut direct = None;
        if let (Some(bc), Some(bl)) = (board_comp, board_lib) {
            for p in bl.pins.iter().filter(|p| p.kind == "io") {
                let n = nets.find(&node(bc, &p.name));
                if n == a || n == b {
                    direct = Some(p.name.clone());
                }
            }
        }
        let hot = [&a, &b]
            .into_iter()
            .find(|n| volts.contains_key(*n))
            .cloned();
        let mosfet = e
            .components
            .iter()
            .filter_map(|m| get(m).and_then(|ml| ml.mosfet.map(|mf| (m, mf))))
            .find(|(m, _)| {
                let dn = nets.find(&node(m, "D"));
                dn == a || dn == b
            });
        if let (Some(pin), None) = (&direct, &mosfet) {
            path.board.clone_from(&board);
            path.pin = Some(pin.clone());
            let i = logic / em.resistance_ohm;
            path.current = board_lib.and_then(|l| l.pin_abs_max_ma).unwrap_or(40.0) / 1000.0;
            path.reason = format!("ligado direto no pino {pin}: pediria {i:.1} A, mas o pino só dá uns {:.0} mA (e queima).", path.current * 1000.0);
            r.issues.push(issue(Severity::Error, "ematchOnPin", format!("O e-match {} está ligado direto no pino {pin} da placa: o pino não fornece a corrente de disparo ({:.1} A) e pode queimar.", c.id, em.all_fire_a), "Comande o e-match por um MOSFET de nível lógico ligado à bateria.", vec![c.part.clone()]));
        } else if let Some((m, mf)) = mosfet {
            let source_gnd = nets.find(&node(m, "S")) == gnd;
            let gate = nets.find(&node(m, "G"));
            let gate_pin = match (board_comp, board_lib) {
                (Some(bc), Some(bl)) => bl
                    .pins
                    .iter()
                    .filter(|p| p.kind == "io")
                    .find(|p| nets.find(&node(bc, &p.name)) == gate)
                    .map(|p| p.name.clone()),
                _ => None,
            };
            path.board.clone_from(&board);
            path.pin.clone_from(&gate_pin);
            let v = hot
                .as_ref()
                .and_then(|n| volts.get(n))
                .copied()
                .unwrap_or(0.0);
            let rint = batteries
                .iter()
                .find(|(bc, _)| hot.as_ref() == Some(&nets.find(&node(bc, "+"))))
                .map_or(main_batt.map_or(1.0, |b| b.internal_ohm), |(_, b)| {
                    b.internal_ohm
                });
            let rds = if logic + 0.05 >= mf.vgs_on {
                mf.rds_on
            } else {
                10.0
            };
            let i = if source_gnd && gate_pin.is_some() {
                v / (em.resistance_ohm + rint + rds)
            } else {
                0.0
            };
            path.current = i;
            path.fires = i >= em.all_fire_a;
            path.reason = if gate_pin.is_none() {
                "o gate do MOSFET não está ligado a um pino da placa".into()
            } else if !source_gnd {
                "o source do MOSFET não está no negativo da bateria".into()
            } else if hot.is_none() {
                "o outro lado do e-match não está no positivo da bateria".into()
            } else {
                format!(
                    "{i:.2} A no disparo (garantido a partir de {:.2} A)",
                    em.all_fire_a
                )
            };
            if rds > mf.rds_on {
                r.issues.push(issue(Severity::Warning, "mosfetGate", format!("A placa de {logic:.1} V não satura o MOSFET (precisa de {:.1} V no gate): a corrente de disparo cai muito.", mf.vgs_on), "Use um MOSFET para 3,3 V ou um driver de gate.", vec![m.part.clone()]));
            }
            if !path.fires {
                r.issues.push(issue(
                    Severity::Error,
                    "ematchCurrent",
                    format!("O e-match {} não acende: {}.", c.id, path.reason),
                    "Confira a fiação e use bateria com menor resistência interna.",
                    vec![c.part.clone()],
                ));
            }
            // Brownout: a placa na mesma bateria que o disparo.
            if let (Some(bc), Some(bl)) = (board_comp, board_lib) {
                let vin = bl
                    .pins
                    .iter()
                    .find(|p| p.kind == "vin")
                    .map(|p| nets.find(&node(bc, &p.name)));
                if vin.is_some() && vin == hot {
                    let sag = v - i * rint;
                    if bl.supply.is_some_and(|s| sag < s.min) {
                        path.brownout = true;
                        r.issues.push(issue(Severity::Warning, "brownout", format!("No disparo, a bateria cai para {sag:.1} V (resistência interna {rint:.2} Ω × {i:.1} A): a placa reinicia (brownout)."), "Use bateria separada para a pirotecnia ou capacitor de reserva na placa.", vec![bc.part.clone()]));
                    }
                }
            }
        } else {
            path.reason = "não há MOSFET nem pino comandando o e-match".into();
            r.issues.push(issue(Severity::Error, "ematchNotWired", format!("O e-match {} não está ligado a nenhum circuito de disparo.", c.id), "Ligue: bateria(+) → e-match → MOSFET (dreno); source no GND; gate num pino da placa.", vec![c.part.clone()]));
        }
        r.pyro.push(path);
    }
    // Autonomia.
    r.standby_ma = standby;
    if let Some((bc, b)) = batteries.first() {
        let charge = bc.charge.unwrap_or(1.0).clamp(0.0, 1.0);
        r.autonomy_h = if standby > 0.0 {
            b.capacity_mah * charge / standby
        } else {
            f64::INFINITY
        };
        if r.autonomy_h < e.standby_hours {
            r.issues.push(issue(Severity::Error, "battery", format!("A bateria dura {:.1} h com {standby:.0} mA, menos que as {:.1} h de espera na rampa.", r.autonomy_h, e.standby_hours), "Use bateria maior ou carregada.", vec![bc.part.clone()]));
        } else if r.autonomy_h < e.standby_hours + 1.0 {
            r.issues.push(issue(
                Severity::Warning,
                "battery",
                format!(
                    "Autonomia de {:.1} h, pouca folga sobre as {:.1} h de espera.",
                    r.autonomy_h, e.standby_hours
                ),
                "Use bateria maior.",
                vec![bc.part.clone()],
            ));
        }
    }
    // Firmware.
    if let Some(f) = &e.firmware {
        let mcu = board_lib.and_then(|l| l.mcu.clone()).unwrap_or_default();
        if mcu != "atmega328p" {
            r.issues.push(issue(
                Severity::Warning,
                "noEmulation",
                format!(
                    "O firmware \"{}\" é para {}, que não tem emulação; use hardware-in-the-loop.",
                    f.name,
                    board_lib.map_or("placa desconhecida", |l| l.name.as_str())
                ),
                "Use Uno, Nano ou Pro Mini para emular, ou ligue a placa real por USB.",
                vec![],
            ));
        }
    }
    r
}

/// Instante em que o firmware disparou o e-match `id`, se disparou e se o circuito acende.
#[must_use]
pub fn pyro_time(d: &Design, id: &str) -> Option<f64> {
    d.electronics
        .pyro_events
        .iter()
        .filter(|p| p.ematch == id)
        .map(|p| p.t)
        .fold(None, |a: Option<f64>, t| Some(a.map_or(t, |x| x.min(t))))
}
