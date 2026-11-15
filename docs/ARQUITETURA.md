# Arquitetura do ASTRA

## 1. Decisões de stack

| Camada | Escolha | Por quê |
| --- | --- | --- |
| Núcleo de física | **Rust → WebAssembly** | `f64` determinístico e idêntico no navegador, no desktop e na CI; Monte Carlo rápido; o mesmo código roda nativo nos testes e benchmarks. |
| Ponte | `wasm-bindgen` (alvo `web`), Web Workers | A UI nunca trava; um worker por rodada de Monte Carlo. |
| Frontend | TypeScript estrito + React + Vite | Como pedido. |
| 3D | CesiumJS (globo, terreno) + Three.js (foguete, lançador, efeitos) | Como pedido. Entra na Fase 4. |
| Gráficos 2D | uPlot | Mais leve que ECharts para séries temporais longas com cursor sincronizado. |
| Firmware | avr8js (AVR). ESP32/STM32/Teensy: ver limitações | Entra na Fase 7. |
| Desktop / offline | Tauri + PWA | Entra na Fase 12. |
| Testes | `cargo test` (física), Vitest (ponte e UI), Playwright (fluxos) | |

Mudanças em relação ao prompt:

- **Sem threads dentro do WASM.** O paralelismo vem de vários workers, cada um com uma
  instância do núcleo. Evita exigir `SharedArrayBuffer` e cabeçalhos COOP/COEP, que quebram
  hospedagem simples e alguns embeds.
- **Sem dependência de álgebra linear externa no núcleo.** `Vec3`, `Quat` e matrizes 3×3 são
  próprios: menos WASM, nenhum risco de mudança de arredondamento entre versões.

## 2. Regras do núcleo

1. SI em toda interface pública. Conversão de unidade só existe na UI.
2. Determinístico: mesma entrada, mesma saída, bit a bit. Aleatoriedade (Monte Carlo,
   turbulência, ruído de sensor) usa gerador próprio com semente explícita na entrada.
3. Sem E/S: o núcleo não lê arquivo, rede ou relógio. Dados (vento real, efemérides, motores)
   entram como parâmetros já carregados pela camada de fora.
4. Toda equação cita a fonte no comentário; toda simplificação entra em `docs/MODELOS.md`.
5. A fronteira (`astra-wasm`) não contém física.

## 3. Estrutura de pastas

Só existe no repositório o que já tem implementação real. A coluna "Fase" diz quando cada
parte nasce.

```
astra/
├─ crates/
│  ├─ astra-math/        Fase 0   Vec3, Quat, constantes (feito)
│  ├─ astra-wasm/        Fase 0   fronteira WASM (feito; cresce a cada fase)
│  ├─ astra-env/         Fase 1   atmosfera, gravidade, referenciais, vento, efemérides
│  ├─ astra-dynamics/    Fase 1   integradores, eventos, 3-DOF, 6-DOF
│  ├─ astra-aero/        Fase 2   Barrowman, Cd(Mach, α, Re), aquecimento, recuperação
│  ├─ astra-propulsion/  Fase 2   sólido, água; Fase 5: líquido, híbrido e demais
│  ├─ astra-vehicle/     Fase 3   peças, materiais, juntas, grafo, validação, falha
│  ├─ astra-launch/      Fase 6   contrato LaunchSystem e modos nativos
│  ├─ astra-avionics/    Fase 7   sensores, circuito, barramentos
│  ├─ astra-mission/     Fase 5   estágios, guiagem, GNC, sequência de voo
│  ├─ astra-orbit/       Fase 10  2 corpos + J2, Lambert, N-corpos
│  ├─ astra-analysis/    Fase 11  Monte Carlo, otimizador, calibração
│  └─ astra-cli/         Fase 1   execução sem UI para benchmarks e validação
├─ packages/
│  ├─ core-js/           Fase 0   cliente tipado do worker (feito)
│  ├─ schema/            Fase 3   formato do projeto (.astra, JSON versionado)
│  ├─ plugin-sdk/        Fase 6   contrato e sandbox de plugins de lançador
│  └─ finance/           Fase 9   BOM, orçamento, câmbio (TypeScript puro)
├─ apps/
│  ├─ web/               Fase 0   React + Vite (feito: casca que carrega o núcleo)
│  └─ desktop/           Fase 12  Tauri
├─ data/                 Fase 2+  motores, materiais, locais, cada item com fonte
├─ docs/
└─ scripts/
```

Dependências entre crates apontam só para baixo:
`math ← env ← dynamics ← {aero, propulsion} ← vehicle ← {launch, avionics} ← mission ← {orbit, analysis} ← wasm/cli`.

## 4. Fluxo de dados

```
UI (React) ──projeto JSON──▶ CoreClient ──postMessage──▶ Worker ──▶ astra-wasm ──▶ núcleo
    ▲                                                                              │
    └──── eventos + telemetria em Float64Array (transferível, sem cópia) ◀─────────┘
```

- O **projeto** (foguete, lançador, eletrônica, local, clima) é um documento JSON versionado.
  É a única entrada do núcleo e o que o autosave, o link compartilhável e o desfazer guardam.
- A **telemetria** volta como colunas `Float64Array` mais uma lista de eventos. Gráficos, HUD,
  linha do tempo e 3D leem a mesma tabela, por isso ficam sincronizados.
- **Corpos múltiplos:** a simulação mantém uma lista de corpos rígidos. Separação de estágio e
  quebra de junta são o mesmo mecanismo: um corpo vira dois, e cada um continua integrando.

## 5. Como o firmware entra no laço (Fase 7)

O emulador roda em TypeScript (avr8js) e a física em Rust. Os dois avançam em passos
travados: a física integra até o próximo instante de amostragem, entrega as leituras dos
sensores, o emulador executa os ciclos de CPU desse intervalo e devolve o estado dos pinos,
que viram comandos (ignitor, servo) para o passo seguinte. Latência e taxa de amostragem
ficam fisicamente corretas por construção.

## 6. Testes e CI

- Cada modelo físico entra com teste contra solução analítica ou tabela publicada.
- `cargo fmt`, `clippy -D warnings`, `cargo test`, `tsc`, Vitest e build do app rodam em todo
  push e PR (`.github/workflows/ci.yml`).
- Benchmarks contra voos reais e outros simuladores ficam em `astra-cli` e alimentam
  `docs/VALIDACAO.md`.
