# Validação

## Testes analíticos

| Teste | Referência | Tolerância | Resultado |
| --- | --- | --- | --- |
| Rotação de 90° em z leva x em y | Definição | 1e-15 | Passa |
| Rotação preserva norma e é invertida pelo conjugado | Definição | 1e-14 | Passa |
| Composição de quaternions = rotações em sequência | Definição | 1e-14 | Passa |
| Cinemática integrada (RK4, 1000 passos) = rotação exata de 180° | Solução fechada para ω constante | 1e-10 | Passa |
| Ida e volta de rotação dentro do runtime WASM (vetor de 6,4e6 m) | Identidade | 1e-8 m | Passa (2,3e-10 m) |

## Benchmarks contra voos reais e outros simuladores

Ainda não há: começam na Fase 2, quando existir voo completo para comparar.
