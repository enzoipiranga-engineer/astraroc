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
- **Fim da queima por tempo.** O motor de teste (`ConstantVehicle`) tem empuxo em degrau;
  curvas reais chegam na Fase 2.
- **6-DOF sem amortecimento de jato** (momento causado pela saída dos gases durante a
  rotação). Entra junto com a propulsão, na Fase 2.
- **Dois cruzamentos do mesmo evento dentro de um único passo passam despercebidos.**
  `h_max` limita essa janela.
- **RK4 fixo não detecta eventos.** Serve para comparação e verificação de ordem.
