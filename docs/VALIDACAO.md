# Validação

## Testes analíticos

| Teste | Referência | Tolerância | Resultado |
| --- | --- | --- | --- |
| Rotação de 90° em z leva x em y | Definição | 1e-15 | Passa |
| Rotação preserva norma e é invertida pelo conjugado | Definição | 1e-14 | Passa |
| Composição de quaternions = rotações em sequência | Definição | 1e-14 | Passa |
| Cinemática integrada (RK4, 1000 passos) = rotação exata de 180° | Solução fechada para ω constante | 1e-10 | Passa |
| Ida e volta de rotação dentro do runtime WASM (vetor de 6,4e6 m) | Identidade | 1e-8 m | Passa (2,3e-10 m) |
| Raios equatorial e polar, ida e volta geodésico ↔ ECEF | WGS 84 | 1e-6 m | Passa |
| Gravidade efetiva no equador e no polo | Gravidade normal WGS 84 (9,7803253359 e 9,8321849379 m/s²) | 2e-4 m/s² | Passa |
| T e p nas 8 bases de camada da atmosfera; ρ, a e μ ao nível do mar | USSA 1976, Tabela 4 | 2e-5 relativo | Passa |
| Ordem de convergência do RK4 | 4 | ±0,1 | Passa |
| Erro do Dormand-Prince acompanha a tolerância (10 períodos do oscilador) | Solução exata | 100 × tol | Passa |
| Saída densa dentro dos passos | cos t | 1e-8 | Passa |
| Instante de impacto e de apogeu por evento (queda livre) | Fórmulas fechadas | 1e-9 s | Passa |
| Tiro vertical no vácuo: apogeu por conservação de energia | v²/2 − μ/r | 1e-3 m em 51 km | Passa |
| Tiro a 45° no vácuo: alcance e tempo de voo | v²·sin2θ/g e 2v·sinθ/g | 5e-5 relativo | Passa |
| Equação do foguete | Δv = v_e·ln(m0/mf) | 1e-6 m/s em 1833 m/s | Passa |
| Empuxo vs. pressão ambiente | F = F_vác − p·A_e | 1e-9 N | Passa |
| Velocidade de saída do trilho com atrito | √(2aL) | 1e-5 relativo | Passa |
| Foguete com empuxo menor que o peso fica na base (Terra girando, 10 min) | Repouso relativo | 1e-3 m | Passa |
| Desvio de Coriolis para leste em queda de 100 m | (1/3)·ω·g·t³ = 21,9 mm | 1e-3 relativo | Passa |
| Velocidade terminal com arrasto | √(2mg/(ρ·CdA)) | 2e-3 relativo | Passa |
| Órbita circular fecha após um período de Kepler; energia e momento angular | 2π√(a³/μ) | 0,05 m; 1e-10 relativo | Passa |
| Regressão dos nodos por J2 (30 órbitas) | −1,5·n·J2·(Re/a)²·cos i | 1% | Passa |
| Pião simétrico sem torque: precessão, L e energia | Goldstein §5.6 | 1e-8 | Passa |
| Corpo assimétrico capotando conserva L | Conservação | 1e-7 | Passa |
| 6-DOF igual ao 3-DOF no trilho; L inercial conservado depois | Consistência | 1e-9 s; 1e-12 | Passa |
| Atmosfera e voo 3-DOF rodando dentro do WASM | Mesmos valores do nativo | — | Passa |
| Gatilhos por tempo, após o apogeu e por altitude | Instantes exatos | 1e-9 s; 1e-6 m | Passa |
| CNα = 2 e CP de cinco perfis de nariz | Barrowman; volumes fechados | 1e-4 (ogiva 0,466L: 2e-3) | Passa |
| Transição cônica: CNα e CP | Fórmulas de Barrowman | 1e-6 m | Passa |
| Foguete de três aletas: CNα e CP | Equações fechadas de Barrowman, à mão | 1e-9; 0,03 em 24,7 | Passa |
| Aleta elíptica: área e CP | 0,288·cr | 1e-12 | Passa |
| CNα das aletas e Cd contínuos em Mach (0 a 4) | Sem saltos | 3% e 6% por 0,01 de Mach | Passa |
| Atrito, base e aletas em M = 0 | Cálculo à mão | 1e-12 (atrito: entre limites) | Passa |
| Arrasto de cone supersônico; boattail longo e curto | Fórmulas | 1e-12 | Passa |
| Tabela isentrópica A/A* em M = 3 | Anderson, Tabela A.1 | 2e-5 | Passa |
| Coeficiente de empuxo de tubeira adaptada | Cálculo à mão (1,5965) | 2e-4 | Passa |
| Arquivo .eng do Cesaroni M1670: cabeçalho, impulso, classe | Arquivo do fabricante; trapézios à mão (6026,35 N·s) | 0,01 N·s | Passa |
| Vazão integrada = massa de propelente | Conservação | 1e-12 kg | Passa |
| .eng exportado e relido; erros com número da linha; .rse | Ida e volta | 0,01 N·s | Passa |
| BATES: área de queima, Kn, equilíbrio de vazão, massa | Fórmulas; conservação | 1e-9 | Passa |
| Água a pressão constante: F = 2·A·Δp e tempo de esvaziamento | Bernoulli | 1e-3 | Passa |
| Água: pressão final adiabática, impulso = ∫√(2ρΔp)dV, massa e empuxo inicial do ar | Quadratura independente; escoamento bloqueado à mão | 2e-3; 1e-9 | Passa |
| Massa, CG e inércia do foguete montado | Cálculo à mão | 1e-12 | Passa |
| Foguete estável: α < 3° em voo; 6-DOF = 3-DOF no apogeu | Consistência | 1% | Passa |
| Foguete instável capota e perde mais de 40% do apogeu | Consequência física | — | Passa |
| Dual deploy: velocidades de descida | √(2mg/(ρ·CdS)) | 3% | Passa |
| Volume ótimo de água | Regra prática de ~1/3 | Entre 20% e 45% (resultado: 29%) | Passa |

## Benchmarks contra voos reais e outros simuladores

| Caso | Referência | ASTRA | Diferença | Observação |
| --- | --- | --- | --- | --- |
| Calisto com motor M1670, Spaceport America, trilho de 5,2 m a 85°, Cd de tabela | RocketPy: apogeu da ordem de 3,3 km acima do solo | 3306 m | ~0% | Comparação entre simuladores, não com voo medido. O valor do RocketPy é o lembrado da documentação (não conferido nesta sessão) e usa atmosfera de previsão; aqui, atmosfera padrão sem vento. |
| Mesmo voo com o Cd analítico do ASTRA no lugar da tabela | Tabela de Cd do Calisto | 3192 m | −3,4% | Mede o erro do modelo de arrasto neste foguete subsônico. |
| Garrafa PET de 2 L, 0,66 L de água, 4 bar, 150 g vazia | Sem dado medido disponível | 47,5 m | — | **Pendente:** o critério "contra dado medido" não foi cumprido. Faltam voos com altímetro. |
| OpenRocket, RASAero II, V-2, Falcon 9, Saturn V | — | — | — | Pendentes (Fases 5 e 12). |
