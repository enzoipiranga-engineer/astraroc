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

## Simplificações em vigor

- **ECI simplificado.** O referencial inercial coincide com o ECEF girado de θ0 + ω·t em z.
  Não há precessão, nutação nem movimento do polo, e θ0 ainda não vem da data/hora (Fase 4).
  Efeito desprezível em voos suborbitais; em órbita, erro de orientação absoluta de até
  dezenas de km por não usar o tempo sideral real.
- **Atmosfera gira com o planeta, sem vento.** Vento entra na Fase 4.
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
