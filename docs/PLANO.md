# Plano por fases

Uma fase só fecha quando o critério de saída passa na CI.

| Fase | Entrega | Critério de saída | Estado |
| --- | --- | --- | --- |
| 0 | Arquitetura, monorepo, CI, base matemática, ponte WASM, casca do app | Testes Rust e JS passam; app carrega o núcleo num worker | **Feita** |
| 1 | Referenciais (ECI/ECEF/ENU/corpo), WGS84, gravidade com J2, atmosfera USSA 1976 até 86 km, RK4 e Dormand-Prince com eventos, 3-DOF e 6-DOF, trilho | Balística no vácuo, Tsiolkovsky, tabela ISA, período orbital e conservação de energia dentro da tolerância | Próxima |
| 2 | Barrowman estendido, Cd(Mach, α, Re), motor sólido (.eng/.rse e grão BATES), foguete de água, paraquedas | Casos de CP da literatura; curva de motor comercial reproduzida; voo de garrafa PET contra dado medido | |
| 3 | Peças, materiais, juntas, grafo, validação contínua, quebra de junta, editor e gráficos 2D | Coifa solta gera aviso e, se ignorado, dois corpos com trajetórias próprias | |
| 4 | Globo multi-escala, locais, vento manual e real, câmeras, HUD, linha do tempo, mapa de recuperação | Voo pequeno visível no terreno real; ponto de queda com distância e azimute | |
| 5 | Líquido e híbrido, multi-estágio, boosters, TVC/GNC, Falcon 9 e Starship | Empuxo vs. pressão ambiente; Δv de estágios contra dados públicos | |
| 6 | Lançadores plugáveis: modos 1 a 4, editor visual, comparação com/sem; depois modos 5 a 9 e plugins | Conservação de energia e momento em cada modo; sandbox bloqueia rede e arquivo | |
| 7 | Eletrônica, fiação, firmware AVR emulado, sensores com ruído, hardware-in-the-loop | Sketch de altímetro detecta apogeu e aciona a ejeção na simulação | |
| 8 | Carga útil, ambiente de voo, regras de competição | Relatório aprovado/reprovado para o preset Zyrion | |
| 9 | BOM, orçamento, receitas, Pareto, exportação | Troca de peça atualiza custo, massa e apogeu | |
| 10 | Órbita, reentrada, pouso propulsivo, estrutura e aquecimento; depois Lua e Marte | Hohmann e Lambert contra solução analítica | |
| 11 | Monte Carlo, otimizador, calibração com telemetria, importação/exportação | Centenas de rodadas em paralelo; .ork importado | |
| 12 | Validação contra voos reais, polimento, desempenho, documentação, PWA e Tauri | Apogeu dentro de ±5% nos voos pequenos de referência | |

## Fase 1 em detalhe

1. `astra-env`: conversões geodésica ↔ ECEF ↔ ECI ↔ ENU; gravidade pontual e com J2;
   USSA 1976 (camadas 0 a 86 km) com desvio de temperatura e pressão local.
2. `astra-dynamics`: RK4 fixo; Dormand-Prince 5(4) com controle de passo e saída densa;
   detecção de evento por troca de sinal com refinamento por bisseção na saída densa.
3. 3-DOF (massa pontual, massa variável) e 6-DOF (quaternion, tensor de inércia e CG móveis).
4. Fase de trilho: vínculo a um grau de liberdade, atrito, inclinação e azimute.
5. `astra-cli`: roda um caso e imprime a telemetria em CSV.
6. Testes: alcance e tempo de voo balístico no vácuo; Δv de Tsiolkovsky; tabela ISA a cada
   camada; período de órbita circular; energia e momento angular conservados em órbita sem
   arrasto; ordem de convergência dos integradores.

## Pontos onde o prompt pede mais do que os dados públicos permitem

Ficam registrados aqui para não virarem promessa falsa:

- **"Milhares de materiais, cada dado com fonte".** MatWeb e MMPDS não permitem
  redistribuição. O banco embarcado nasce com algumas centenas de materiais de fontes abertas
  (NIST, MIL-HDBK-5J público, fichas de fabricantes) e cresce por importação do usuário.
- **Motores reais (Raptor, BE-4 etc.).** Parte dos números públicos é estimativa de terceiros;
  cada campo leva a marca "medido / catálogo / estimado".
- **ESP32, STM32 e Teensy.** Não há emulador de navegador com a fidelidade do avr8js. Entram
  primeiro como componentes elétricos (massa, consumo, pinos) e por hardware-in-the-loop;
  firmware emulado fica restrito a AVR até haver emulador confiável.
- **Terreno e imagem de satélite** dependem de um provedor (Cesium ion ou outro) e de chave
  do usuário; offline só com dados em cache.
- **±5% de apogeu** é meta para voos pequenos com Cd e motor bem conhecidos. Para voos
  orbitais, a validação compara eventos (MECO, separação, inserção) com dados públicos.
