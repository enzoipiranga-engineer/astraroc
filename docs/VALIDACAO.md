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

## Benchmarks contra voos reais e outros simuladores

Ainda não há: começam na Fase 2, quando existir voo completo para comparar.
