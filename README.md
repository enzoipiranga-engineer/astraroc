# ASTRA

Simulador de foguetes com física real, da garrafa PET ao Starship. Núcleo em Rust compilado
para WebAssembly, interface em React, globo 3D com CesiumJS. Roda no navegador (inclusive
offline, como aplicativo instalável) e como programa de computador (Tauri).

> Ferramenta de estudo e projeto. Não substitui análise de segurança, autorização de espaço
> aéreo nem as normas de lançamento locais.

## O que ele faz

- **Gêmeo digital do foguete:** peças com material, massa e resistência; juntas com
  capacidade; a falha é simulada (coifa que solta, aleta que arranca, tubo que flamba,
  cordão que rompe) e cada pedaço segue a própria trajetória.
- **Física:** 3-DOF e 6-DOF com Terra elíptica girando, J2, atmosfera padrão de 1976,
  vento em camadas e rajadas, Barrowman estendido, arrasto por componente, motores sólidos
  (.eng/.rse, grão BATES), foguete de água, líquidos, híbridos, multiestágio, TVC.
- **Verificação contínua** em linguagem simples, com sugestão de correção.
- **Globo 3D** com o local real, câmeras, HUD, linha do tempo e mapa de recuperação.
- **Lançadores plugáveis:** rampa, canhão de ar, railgun, plataforma giratória, avião,
  balão, navio, submarino e plugins próprios.
- **Eletrônica e Arduino:** fiação com verificação elétrica, firmware .hex do ATmega328P
  emulado no voo com BMP280, MPU-6050 e GPS virtuais; hardware-in-the-loop por USB.
- **Carga útil e competições:** ambiente de voo (g, choque, temperatura, pressão na baia,
  ruído), regras configuráveis com pontuação (presets SA Cup/IREC 2024 e LASC).
- **Finanças:** lista de materiais, orçamento por subsistema, receitas, Pareto de custo ×
  desempenho, exportação .xlsx/CSV/PDF.
- **Espaço:** Hohmann, Lambert, transferências interplanetárias com janelas, pontos de
  Lagrange, reentrada com aquecimento, pouso propulsivo na Lua, Marte e Terra.
- **Análises:** Monte Carlo em paralelo com elipse de dispersão, sensibilidade, otimizador,
  calibração com telemetria, importação do OpenRocket (.ork).
- **Modelos prontos:** garrafa PET, kit 18 mm, níveis 1–3, competição (com aviônica e carga
  útil), Falcon 9, Starship e Saturn V; regras da LASC (RCSM ed. 7) e da SA Cup/IREC.
- **Exportação:** KML/KMZ, CSV, Tacview, vídeo MP4/WebM/GIF, .eng, planilha e PDF.
- **Português e inglês**, unidades SI e americanas.

O estado de cada fase e o que ficou para depois estão em [docs/PLANO.md](docs/PLANO.md);
as simplificações de cada modelo, em [docs/MODELOS.md](docs/MODELOS.md).

## Requisitos

- Rust estável com o alvo `wasm32-unknown-unknown`
- `wasm-bindgen-cli` 0.2.130 (`cargo install wasm-bindgen-cli --version 0.2.130 --locked`)
- Node 22 e pnpm 10
- Para o app de computador no Linux: `libwebkit2gtk-4.1-dev` e `librsvg2-dev`
  (ver [docs/DESKTOP.md](docs/DESKTOP.md))

## Comandos

| Comando | O que faz |
| --- | --- |
| `pnpm install` | Instala as dependências JS |
| `pnpm dev` | Compila o núcleo para WASM e abre o app |
| `pnpm test` | Testes do núcleo (Rust) e da ponte JS |
| `pnpm typecheck` | Checagem de tipos do TypeScript |
| `pnpm build` | Build de produção do app web (com service worker para uso offline) |
| `pnpm desktop` | Build do app de computador (Tauri) |

## Documentos

- [Guia de uso](docs/USO.md)
- [Arquitetura](docs/ARQUITETURA.md)
- [Plano por fases](docs/PLANO.md)
- [Modelos físicos e limitações](docs/MODELOS.md)
- [Validação](docs/VALIDACAO.md)
- [Dados pendentes](docs/PENDENCIAS.md)
- [Hardware-in-the-loop](docs/HIL.md)
- [App de computador](docs/DESKTOP.md)

## Licença

MIT. Veja [LICENSE](LICENSE).
