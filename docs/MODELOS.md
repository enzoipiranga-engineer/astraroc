# Modelos físicos, validade e limitações

Cada modelo implementado ganha uma linha aqui no mesmo commit do código.

| Modelo | Onde | Fonte | Faixa de validade | Erro esperado |
| --- | --- | --- | --- | --- |
| Rotação por quaternion (Hamilton, corpo → referência) | `astra-math::Quat::rotate` | Diebel 2006; Stevens & Lewis §1.8 | Quaternion unitário | Arredondamento de `f64` (~1e-16 relativo) |
| Cinemática de atitude q̇ = ½ q ⊗ ω | `astra-math::Quat::derivative` | Stevens & Lewis, eq. 1.8-15 | Qualquer ω; exige renormalização periódica | Depende do integrador; norma deriva sem renormalizar |
| Elipsoide e constantes WGS 84 | `astra-math::constants::wgs84` | NGA.STND.0036_1.0.0 | Terra | Constantes de definição |
| Geodésico ↔ ECEF | `astra-env::Planet` | NGA.STND.0036 eq. 4-14; Hofmann-Wellenhof §10.2 | Qualquer altitude acima de ~−1000 km | < 1e-6 m (teste de ida e volta) |
| Gravidade central + J2 | `astra-env::Planet::gravity` | Vallado cap. 8; Bate, Mueller & White §9 | Qualquer r > 0 | ~1e-4 m/s² na superfície (faltam J3, J4 e anomalias locais) |
| Atmosfera USSA 1976, 0 a 86 km | `astra-env::AtmosphereModel` | NOAA-S/T 76-1562, Tabela 4, eqs. 18, 33, 50, 51 | −5 a 86 km | < 2e-5 relativo em pressão nas bases das camadas |
| Atmosfera 86 a 1000 km (faixas exponenciais) | idem | Vallado, Tabela 8-4 | 86 a 1000 km, atividade solar média | Fator 2 ou mais conforme o ciclo solar; salto de ~3% em 86 km |
| Runge-Kutta 4 fixo | `astra-dynamics::ode::rk4_step` | Hairer, Nørsett & Wanner §II.1 | Problemas não rígidos | O(h⁴) global (ordem medida: 4,0) |
| Dormand-Prince 5(4) com saída densa | `astra-dynamics::ode::integrate` | Dormand & Prince 1980; Hairer et al. §II.4 a II.6 | Problemas não rígidos | Controlado por `rtol`/`atol` |
| Detecção de eventos | idem | Bisseção sobre a saída densa | Um cruzamento por evento por passo | ~1e-9 s no instante |
| Voo 3-DOF | `astra-dynamics::flight::simulate3` | Zipfel cap. 8 | Veículo com ângulo de ataque desprezível | Depende do Cd e do empuxo fornecidos |
| Voo 6-DOF de massa variável | `astra-dynamics::flight::simulate6` | Stevens & Lewis eqs. 1.7-5, 1.8-15, 1.8-16; Zipfel §6.3 | Corpo rígido | Depende das cargas fornecidas |
| Trilho (1 grau de liberdade, atrito de Coulomb) | `flight::World::rail_accel` | Goldstein §4.10 | Trilho reto e rígido | < 1e-5 na velocidade de saída (teste) |
| Empuxo vs. pressão ambiente | `flight::ConstantVehicle` | Sutton & Biblarz eq. 2-13 | Escoamento não separado na tubeira | Exato para o modelo |
| CNα e CP do corpo (nariz, transições) | `astra-aero::Geometry::linear_components` | Barrowman (1967), teoria de corpo esbelto | Subsônico, α pequeno, corpo esbelto | CP em ~0,1 a 0,3 calibre |
| CNα das aletas (trapezoidal, elíptica) com compressibilidade | `astra-aero::FinSet::single_fin_cn_alpha` | Barrowman (1967); Diederich NACA TN-2335; teoria linear supersônica | M ≤ 0,9 e M ≥ 1,5 confiáveis; 0,9 a 1,5 interpolado | Subsônico ~5%; transônico até ~30% |
| Sustentação de corpo com α | `astra-aero::Geometry::normal_force` | Galejs (1999) | α até ~15° | ~20% do termo |
| Amortecimento de arfagem | `astra-aero::Geometry::pitch_damping_sum` | Mandell, Caporaso & Bengen (1973) | Quase estático | ~20% |
| Arrasto de atrito | `astra-aero` (`drag.rs`) | Niskanen (2013) cap. 3; Hoerner | Camada limite toda turbulenta | ~10% do termo; subestima trechos laminares em modelos pequenos |
| Arrasto de pressão do nariz e de ombros | idem | Niskanen (2013) cap. 3 | Cone confiável; demais perfis por fator estimado | Subsônico pequeno; supersônico ±30% |
| Arrasto de base e boattail | idem | Niskanen (2013) cap. 3 (ajuste a Hoerner) | Qualquer Mach | ~15% do termo |
| Arrasto de pressão das aletas | idem | Niskanen (2013) cap. 3 | Qualquer Mach | ~20% do termo |
| Motor por tabela (RASP .eng, RockSim .rse) | `astra-propulsion::Motor` | Formato thrustcurve.org | Isp constante quando o arquivo não traz massa | Massa durante a queima ~5% |
| Balística interna de grão BATES | `astra-propulsion::BatesMotor` | Sutton & Biblarz caps. 3 e 12; Nakka | Regime quase permanente, sem queima erosiva | Pressão e empuxo ±10 a 15%; não prevê ignição nem cauda |
| Coeficiente de empuxo isentrópico | `astra-propulsion::thrust_coefficient` | Sutton & Biblarz cap. 3 | Sem separação na tubeira | Ideal; perdas entram por eficiência informada |
| Foguete de água, fase de água | `astra-propulsion::WaterRocket` | Finney (2000); White §3.5 | Bocal simples, sem tubo de lançamento | Impulso ~5 a 10% (coeficiente de descarga) |
| Foguete de água, fase de ar | idem | Anderson, Modern Compressible Flow cap. 5 | Bocal convergente, esvaziamento isentrópico | ~10% do impulso do ar |
| Paraquedas com enchimento e rizadura | `astra-vehicle::Parachute` | Knacke (1992) §5.4 | Tecido sólido, enchimento ∝ t² | Força de abertura ±30% |
| Foguete montado (massa, CG, inércia, cargas 6-DOF) | `astra-vehicle::Rocket` | Steiner; composição dos modelos acima | Um estágio | — |
| Volume ótimo de água | `astra-vehicle::optimal_water_volume` | Seção áurea (Kiefer 1953) | Um único máximo | ~1% do volume |

| Massa, CG e inércia por peça (cascas finas de revolução, cilindros, discos, placas) | `astra-design::PartProps` | Integração numérica; Steiner | Paredes finas frente ao raio | Massa ~2 a 5% (colas, acabamento e furos não entram) |
| Capacidade de colagem | `astra-design::Assembly` | τ = menor resistência ao cisalhamento entre adesivo e peças × área de contato | Sobreposição cilíndrica ou face | ±50%: colagem real depende de preparo e cura |
| Raiz da aleta (filetes como binário; placa σ·c·t²/6) | idem | Modelo de engenharia próprio, documentado no código | Aleta colada na superfície ou atravessando o tubo | ±50% |
| Parafusos (cisalhamento 0,6·σ_rup ou τ do material; esmagamento σ_esc·d·t) | idem | Prática de projeto de juntas (Shigley, cap. 8) | Parede fina | ±30% |
| Pinos de cisalhamento τ·πd²/4 | idem | Resistência ao cisalhamento do material | Pino íntegro e seco | ±30% (náilon varia com umidade) |
| Contato entre peças externas sem junta | idem | Só compressão, tração nula | Sempre | — |
| Carga axial numa junta: F = m_distal·a_x − F_aero,distal | `astra-design` (`limits.rs`) | Segunda lei de Newton no subconjunto | Corpo rígido; arrasto repartido por peça | Igual ao erro do arrasto por peça (~20%) |
| Pressão da baia fechada: Δp·A, com Δp = p_solo − p_ambiente | idem | Hidrostática | Baia sem respiro, sem vazamento | Superestima: baias reais vazam |
| Força de ejeção p·A, p = m·R·T/V (pólvora negra, R·T ≈ 2,19e5 J/kg) | `Assembly::ejection_force` | Regra usual de dimensionamento (R = 22,16 ft·lbf/(lbm·°R), T = 3307 °R) | Gás ideal, sem perdas | Superestima (perdas térmicas e vazamento); estimado |
| Flutter de aleta (NACA TN 4197) | `limits::flutter_speed` | Martin (1958); Apogee Peak of Flight nº 291 | Placa fina isotrópica | ±30% |
| Flambagem local de tubo (com fator γ) e escoamento | `limits::shell_buckling_stress` | NASA SP-8007 | Casca fina, ν = 0,3 | ±30%; não modela flambagem global de coluna |
| Momento fletor no tubo pelas forças normais à frente | `limits.rs` | Barrowman por componente; inércia lateral | α pequeno; sem aceleração angular | ~30% |
| Corpo capotando: Cd ≈ 1 sobre a área projetada média (superfície/4, Cauchy) | `limits::TUMBLE_CD` | Fórmula de Cauchy; Hoerner cap. 3 | Rotação aleatória rápida | ±50% |
| Sistema sob paraquedas como ponto de massa; força de abertura q·CdS(t) contra o cordão mais fraco | `limits::PointSystem` | Knacke (1992) | Corpos amarrados alinhados com o fluxo | Força de pico ±30% |
| Separação: partes herdam r e v do corpo (com ω × d) | `sim.rs` | Cinemática de corpo rígido | Separação instantânea, sem impulso de mola ou carga | Desvio da ordem do impulso desprezado |
| Vento médio por altura (componentes interpoladas linearmente) | `astra-env::Wind` | — | Perfil dado | Igual ao do dado de entrada |
| Lei de potência da camada limite | `Wind::power_law` | Hellmann (1916); ESDU 82026 | Até ~300 m, terreno homogêneo | ±20% |
| Turbulência de Dryden por representação espectral (campo congelado fixo à Terra) | `astra-env::Turbulence` | MIL-HDBK-1797; Shinozuka & Deodatis (1991) | Isotrópica, σ e L constantes | Estatística correta em média; forma transversal de Dryden não modelada |
| Hora sideral média de Greenwich | `astra-env::time::gmst` | Vallado eq. 3-47 (IAU-82) | 1950 a 2100 | < 1e-5° (teste do exemplo 3-5) |
| Perfil de vento previsto/histórico | `apps/web/src/geo.ts` | Open-Meteo (GFS, ICON, ECMWF) | Grade dos modelos (~10 a 25 km), horário | Típico de previsão: 1 a 3 m/s |
| Motor líquido por catálogo: F = acel·F_vác − p_a·A_e, A_e de (F_vác − F_nm)/p_nm | `astra-propulsion::LiquidEngine` | Sutton & Biblarz cap. 2 | Escoamento não separado | Igual ao dado de catálogo |
| Biblioteca de 12 motores reais | `engine_library` | Dados públicos (fabricantes, NASA) | — | Estimado: transcrito de memória, não conferido |
| Desempenho ideal (c*, C_F, Isp) a partir de Tc, M, γ | `ideal_performance` | Sutton & Biblarz cap. 3 | Escoamento congelado; sem equilíbrio químico | 5 a 10% acima do real |
| Motor híbrido de canal único, ṙ = a·G^n | `astra-propulsion::HybridMotor` | Marxman et al. (1964); Sutton cap. 16 | c* constante; sem transitórios | ±20%; coeficientes do exemplo estimados |
| Lançador multiestágio 3-DOF com boosters, carenagem, falha de motor | `astra-mission` | Composição dos modelos acima | Corpo rígido sem atitude | Depende dos dados do veículo |
| Cd genérico de lançador por Mach | `templates::generic_cd` | Forma de curvas publicadas (Saturn V, Atlas) | Lançador esbelto | ±30% (efeito pequeno no Δv) |
| Guiagem: vertical, manobra inicial (escolha automática), gravity turn, tangente linear com tiro (Newton) | `astra-mission::simulate` | Bryson & Ho, "Applied Optimal Control" (tangente linear) | Órbita circular alvo | Perda por direção maior que a de uma guiagem ótima |
| Limites suaves de max-Q e de aceleração | idem | — | — | — |
| Orçamento de Δv (gravidade, arrasto, direção, contrapressão) | idem | d|v|/dt = (T/m)cosθ + D·v̂/m + g·v̂ | Referencial inercial | Fecha em 1% (trapézios) |
| Elementos orbitais RV2COE | `astra-mission::elements` | Vallado alg. 9 | Elíptica ou hiperbólica | Arredondamento |
| TVC: gimbal com controle PD por alocação de polos; giroscópio com viés e ruído de banda limitada | `astra-vehicle::TvcRocket` | Ogata cap. 5; Wie cap. 6 | Ângulos pequenos; atuador ideal | — |
| Guia retilínea 1-D (rampa, trilho, canhão, mola, mão, plugins) | `astra-launch::guided` | Newton com atrito de Coulomb e arrasto | Guia rígida; referencial local plano | Balanço de energia fecha em < 1e-8 |
| Canhão de gás adiabático | `Push::GasGun` | Sutton & Biblarz cap. 3 | Gás ideal, sem atrito do gás | Superestima (perdas reais de 20 a 40%) |
| Railgun F = ½·L'·I² | `Push::Railgun` | Fair, IEEE Trans. Magnetics (1999) | Corrente constante; sem arco nem aquecimento | Ordem de grandeza |
| Plataforma giratória (braço rígido, torque do motor, arrasto do braço) | `astra-launch::spin` | Dinâmica de rotação de corpo rígido | Plano vertical; braço uniforme | Arrasto do braço estimado |
| Lançamento aéreo (soma galileana do estado do avião) | `air_launch` | Cinemática | Separação limpa, sem esteira do avião | — |
| Balão de pressão zero com deriva pelo vento | `balloon` | Yajima et al., "Scientific Ballooning" cap. 2 | Gás em equilíbrio térmico com o ar; sem massa aparente | Velocidade de subida ±20% |
| Navio (convés com arfagem e balanço senoidais) | `ship` | Cinemática | Aceleração do convés desprezada no trilho | — |
| Submarino (ejeção a frio, empuxo e arrasto na água) | `submarine` | Hidrostática; arrasto quadrático | Sem cavitação nem massa adicional | ±30% |
| Cadeia de lançadores | `run_chain` | Composição galileana | — | — |
| Plugins declarativos com expressões seguras | `astra-launch::PluginSpec`, `expr` | — | Guia retilínea com força arbitrária | Verificação de energia e testes do autor |
| Limites de g da estrutura na fase de lançamento | `astra-design` (`structural_g_limits`) | Capacidade dos cortes / (massa · g0) | Carga quase estática | Igual ao das capacidades das juntas |
## Simplificações em vigor

- **ECI simplificado.** O referencial inercial coincide com o ECEF girado de θ0 + ω·t em z,
  com θ0 = hora sideral média de Greenwich da data do lançamento (zero sem data). Não há
  precessão, nutação nem movimento do polo.
  Efeito desprezível em voos suborbitais; em órbita, erro de orientação absoluta de até
  dezenas de km por não usar o tempo sideral real.
- **Vento horizontal só no perfil médio;** componente vertical só pela turbulência. A
  turbulência usa o mesmo espectro para as três componentes e não varia com a altitude.
- **Altura geopotencial tratada como geométrica** ao converter níveis de pressão (erro
  abaixo de 0,3% até 12 km).
- **Acima de 86 km a temperatura fica fixa em 186,87 K** e só a massa específica varia.
  Mach e pressão acima de 86 km são aproximações grosseiras; o arrasto usa só ρ. Será
  substituído por NRLMSISE-00 na Fase 10.
- **Tabela exponencial acima de 86 km transcrita sem acesso ao livro nesta sessão.** Os
  valores precisam ser conferidos com a Tabela 8-4 impressa do Vallado antes de serem
  usados em análise de decaimento orbital.
- **3-DOF com ângulo de ataque nulo.** O empuxo segue a velocidade relativa ao ar; não há
  sustentação nem "weathercocking". Use o 6-DOF quando isso importar.
- **Trilho.** O veículo não desliza para trás; folga das guias e flexão do trilho são
  ignoradas, e no 6-DOF a atitude fica rígida até a saída (sem "tip-off").
- **6-DOF sem amortecimento de jato** (momento causado pela saída dos gases durante a
  rotação). Pequeno frente ao amortecimento aerodinâmico em foguetes com aletas; entra com
  o TVC, na Fase 5.
- **Dois cruzamentos do mesmo evento dentro de um único passo passam despercebidos.**
  `h_max` limita essa janela.
- **RK4 fixo não detecta eventos.** Serve para comparação e verificação de ordem.
- **Números de equação das fontes não foram conferidos.** As equações foram implementadas
  e testadas contra soluções fechadas, mas as fontes são citadas por capítulo porque os
  livros não estavam acessíveis nesta sessão.
- **Dados transcritos de memória, a conferir com a fonte:** tabela exponencial da atmosfera
  acima de 86 km (Vallado) e propriedades e lei de queima do KNSB (Nakka). Não use o motor
  KNSB calculado para dimensionar um motor real sem conferir.
- **Fator de forma do arrasto de onda do nariz é estimado** (cone = 1; ogiva 0,95; parábola
  0,85; LV-Haack 0,78; von Kármán 0,75; elipse 1,3). Para voo supersônico sério, importe
  uma tabela de Cd (`DragModel::Table`).
- **Arrasto não varia com o ângulo de ataque** além da projeção no eixo; o arrasto induzido
  aparece só pela força normal. Vale para α pequeno.
- **Sem rolamento aerodinâmico:** aletas sem incidência e sem momento de rolamento; a taxa
  de rolamento fica constante. Aletas de forma livre, botões de trilho e flutter ficam
  para a Fase 3.
- **CG do propelente fixo** durante a queima, e motor tratado como cilindro homogêneo para
  a inércia.
- **Sob paraquedas, o foguete vira massa pontual:** sem momentos, e a atitude e o ângulo
  de ataque registrados deixam de ter significado. O atraso do altímetro no gatilho por
  altitude não é modelado.
- **Foguete de água:** empuxo não corrigido com a altitude; coluna de água e aceleração
  desprezadas na velocidade de saída; se a pressão acaba antes da água, o resto fica
  como peso morto (na realidade escorre).
- **Motor sólido só com grão BATES.** Estrela e finocyl ficam para a Fase 5, com o projeto
  de motor.
- **Gêmeo digital (fase 3):** colas, tinta, parafusos e fiação não entram na massa
  automática; use "massa medida" para corrigir. A geometria das peças internas é axial
  (cilindros centrados).
- **Cargas nas juntas só axiais**, mais flexão na raiz das aletas e no tubo. Torção e
  cargas de lançador ainda não entram.
- **No trilho, a carga nas juntas usa só empuxo e arrasto** (sem a reação do trilho):
  antes de sair da base a compressão real é um pouco maior.
- **Juntas implícitas:** peças externas encostadas sem junta ficam apoiadas (só
  compressão); peças internas soltas ficam confinadas no tubo que as envolve e não saem
  nem quando a baia abre.
- **Ejeção sem a carga de voo:** a decisão de abrir a baia compara só a força da carga de
  ejeção com a capacidade da junta.
- **Peças separadas** voltam ao 6-DOF só se tiverem nariz e margem acima de 0,3 calibre;
  senão capotam como ponto de massa (sem empuxo útil se o motor ainda queimar).
- **Aleta perdida:** o conjunto passa a ter uma aleta a menos, com o mesmo modelo
  simétrico de Barrowman (a assimetria real e o rolamento que ela causa não entram).
- **Flambagem:** quando um tubo flamba, o foguete inteiro passa a capotar; a quebra do
  tubo em duas partes não é modelada.
- **Visualização sem terreno:** sem chave do Cesium ion, o solo é o elipsoide e a trajetória
  é desenhada pela altura acima do solo. Com terreno, a altura do terreno no local é somada.
- **Mapa de recuperação no plano tangente** (aproximação equirretangular), com erro
  desprezível a poucos quilômetros do lançador.
- **Lançadores em 3-DOF:** sem atitude nem flexão; o empuxo segue a guiagem diretamente.
  Hot staging = ignição do superior no instante da separação, sem sobreposição de empuxo.
- **Estágios descartados** caem capotando, sem aquecimento de reentrada nem queima de
  retorno (fase 10).
- **TVC sem integrador nem dinâmica de atuador** (controle PD; o gimbal responde
  instantaneamente). A malha só age durante a queima e fora do trilho.
- **Sem equilíbrio químico (NASA CEA):** Isp de motores customizados vem de Tc, M e γ
  informados ou da tabela aproximada.
- **Fase de lançamento em referencial local plano** (sem curvatura nem Coriolis), válida até
  dezenas de quilômetros do local; o voo depois da liberação usa o modelo completo.
- **Na fase de lançamento as juntas não quebram**: as cargas são comparadas com o limite
  da estrutura e geram alerta. A falha simulada continua valendo depois da liberação.
- **Plugins só de guia retilínea** nesta versão (o formato é versionado para crescer).
  Scripts TypeScript/WASM arbitrários não são aceitos: no lugar deles há a linguagem de
  expressões, que não tem laços nem acesso a nada fora das variáveis.

## Eletrônica e firmware (fase 7)

- **Redes elétricas** por união de pinos ligados por fios (union-find). Tensão de cada rede:
  a da bateria, depois a das saídas de regulador das placas alimentadas. Verificações:
  sub/sobretensão contra a faixa do componente, GND comum, saída de regulador sobrecarregada,
  corrente de pino de saída, endereço I2C repetido no mesmo barramento, nível lógico
  (sensor de 3,3 V num barramento de 5 V), antena dentro de tubo de carbono (≈ 30 dB) ou
  metal (≈ 60 dB).
- **Circuito pirotécnico:** I = V / (R_ematch + R_interna + R_ds,on). Se o gate recebe menos
  que a tensão de condução plena do MOSFET, R_ds é tomado como 10 Ω (MOSFET mal saturado).
  E-match ligado direto no pino: o pino só fornece a corrente máxima absoluta (≈ 40 mA no
  ATmega328P), muito abaixo do all-fire. **Brownout:** a tensão no disparo cai para
  V − I·R_interna; se ficar abaixo da alimentação mínima da placa ligada na mesma bateria, a
  placa reinicia. Modelo resistivo em regime, sem indutância nem capacitores.
- **Autonomia** = capacidade × carga / soma das correntes típicas (sem picos de rádio).
- **Alcance de rádio:** perda de espaço livre (ITU-R P.525) com 10 dB de margem e antena de
  2,15 dBi em terra; sem relevo, sem polarização, sem a atenuação variável com a atitude.
- **Verdade dos sensores** (a 100 Hz no corpo que leva a placa): pressão e temperatura da
  atmosfera na altitude; força específica f = a − g, com a por diferença central da
  velocidade inercial e g da gravidade J2, girada para o corpo pelo quaternion; velocidade
  angular do corpo. No 3-DOF (sob paraquedas ou capotando) a atitude é desconhecida: o eixo
  x é tomado como a vertical local e a velocidade angular é zero.
- **Sensores virtuais:** ruído gaussiano do datasheet, viés fixo sorteado (acelerômetro
  2 % de g por eixo), saturação no fundo de escala configurado pelo firmware, atraso de meia
  janela de medição do BMP280 (tempo típico do §3.8.1 do datasheet). O BMP280 usa os
  coeficientes de calibração do exemplo do datasheet e as fórmulas inteiras de compensação;
  o valor cru é obtido por bisseção, de modo que o firmware reconstrói a pressão simulada.
- **Erro da porta estática com o Mach** é **ilustrativo**: k = −0,01 subsônico, um pico de
  +0,15·q na faixa transônica (0,85 < M < 1,15) e +0,02·q supersônico. Reproduz o fenômeno
  qualitativo (o altímetro “vê” uma queda de altitude ao passar pelo transônico), mas não é
  medido para uma geometria.
- **Baia sem furo de respiro:** o barômetro lê a pressão do solo durante todo o voo (selagem
  perfeita; vazamento lento não é modelado).
- **GPS:** sentenças GGA a 5 Hz com erro gaussiano; perde a solução acima de 4 g, acima de
  515 m/s e 18 km juntos (COCOM) ou com a antena bloqueada. Sem tempo de reaquisição.
- **Fio mal crimpado** solta (e fica solto) quando a aceleração passa do limite escolhido:
  o sensor deixa de responder; se for alimentação da placa, a placa para.
- **Firmware no laço:** uma única passada do firmware; quando ele aciona um canal
  pirotécnico, a física é refeita com o disparo naquele instante. Como as entradas antes do
  disparo não mudam, o firmware continua de onde estava sem rebobinar. A placa liga 3 s antes
  da ignição. A emulação termina 1 s depois do pouso, 2 s depois do último canal acionado ou
  em 300 s.
- **Fórmula de altitude do firmware de exemplo** (hipsométrica de atmosfera padrão) supõe
  15 °C na pressão de referência; num campo a 470 m o erro é de ≈ 1 % da altura. É um erro
  real dos altímetros barométricos simples e aparece na simulação.
