# Plano por fases

Uma fase só fecha quando o critério de saída passa na CI.

| Fase | Entrega | Critério de saída | Estado |
| --- | --- | --- | --- |
| 0 | Arquitetura, monorepo, CI, base matemática, ponte WASM, casca do app | Testes Rust e JS passam; app carrega o núcleo num worker | **Feita** |
| 1 | Referenciais (ECI/ECEF/ENU/corpo), WGS84, gravidade com J2, atmosfera USSA 1976 até 86 km, RK4 e Dormand-Prince com eventos, 3-DOF e 6-DOF, trilho | Balística no vácuo, Tsiolkovsky, tabela ISA, período orbital e conservação de energia dentro da tolerância | **Feita** |
| 2 | Barrowman estendido, Cd(Mach, α, Re), motor sólido (.eng/.rse e grão BATES), foguete de água, paraquedas | Casos de CP da literatura; curva de motor comercial reproduzida; voo de garrafa PET contra dado medido | **Feita, com uma pendência:** falta voo medido de garrafa PET |
| 3 | Peças, materiais, juntas, grafo, validação contínua, quebra de junta, editor e gráficos 2D | Coifa solta gera aviso e, se ignorado, dois corpos com trajetórias próprias | **Feita** |
| 4 | Globo multi-escala, locais, vento manual e real, câmeras, HUD, linha do tempo, mapa de recuperação | Voo pequeno visível no terreno real; ponto de queda com distância e azimute | **Feita** |
| 5 | Líquido e híbrido, multi-estágio, boosters, TVC/GNC, Falcon 9 e Starship | Empuxo vs. pressão ambiente; Δv de estágios contra dados públicos | **Feita** |
| 6 | Lançadores plugáveis: modos 1 a 4, editor visual, comparação com/sem; depois modos 5 a 9 e plugins | Conservação de energia e momento em cada modo; sandbox bloqueia rede e arquivo | **Feita** |
| 7 | Eletrônica, fiação, firmware AVR emulado, sensores com ruído, hardware-in-the-loop | Sketch de altímetro detecta apogeu e aciona a ejeção na simulação | **Feita** |
| 8 | Carga útil, ambiente de voo, regras de competição | Relatório aprovado/reprovado para o preset Zyrion | **Feita, com uma pendência:** o preset Zyrion espera os dados da equipe; o relatório aprovado/reprovado funciona para qualquer carga |
| 9 | BOM, orçamento, receitas, Pareto, exportação | Troca de peça atualiza custo, massa e apogeu | **Feita** |
| 10 | Órbita, reentrada, pouso propulsivo, estrutura e aquecimento; depois Lua e Marte | Hohmann e Lambert contra solução analítica | **Feita** |
| 11 | Monte Carlo, otimizador, calibração com telemetria, importação/exportação | Centenas de rodadas em paralelo; .ork importado | Próxima |
| 12 | Validação contra voos reais, polimento, desempenho, documentação, PWA e Tauri | Apogeu dentro de ±5% nos voos pequenos de referência | |

## Fase 1 em detalhe (concluída)

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

## Fase 3: o que ficou para depois

- Editor por arrastar e soltar no desenho: hoje as peças são posicionadas por número
  (posição em mm) e selecionadas clicando no desenho ou na lista.
- Gráficos de aceleração, carga em cada junta ao longo do tempo e exportação CSV: os
  dados existem no núcleo; a tela mostra altura, velocidade e ângulo de ataque.
- Idioma inglês, unidades imperiais, link compartilhável e tutorial inicial (fase 12).
- Aletas de forma livre e elípticas no editor (o núcleo aerodinâmico já tem a elíptica).
- Banco com "milhares" de materiais: hoje são 50, cada um com fonte e confiabilidade.
- Teste de navegador fora da CI (precisa de Chromium no executor).

## Fase 4: o que ficou para depois

- Elipse de dispersão 50/90/99%: depende do Monte Carlo (fase 11).
- Exportação de vídeo (MP4/GIF) e várias simulações sobrepostas no globo (fase 11/12).
- Fumaça, diamantes de choque e pluma que muda com a altitude: hoje a pluma é um ponto
  luminoso enquanto o motor queima.
- Modelo 3D detalhado do foguete (Three.js): hoje é um cilindro em escala real orientado
  pela atitude simulada, com marcador e rótulo à distância.
- Lua, planetas e efemérides JPL ficam na fase 10.
- A API de previsão da Open-Meteo não pôde ser chamada desta sessão (limite diário do IP
  compartilhado); a conversão foi testada com uma resposta no formato documentado. A busca
  de locais foi testada de verdade.

## Fase 5: o que ficou para depois

- Equilíbrio químico (NASA CEA) para Isp e razão de mistura ótima: hoje, tabela aproximada.
- Grãos estrela e finocyl, injetor, bomba e projeto completo de motor no app.
- Gás frio, RCS, elétrico, nuclear térmico, ramjet/scramjet.
- PEG completa, reignição e órbitas elípticas alvo (fase 10, com a fase orbital).
- Editor de estágios na tela (hoje: Falcon 9 e Starship com parâmetros da missão; o núcleo
  aceita qualquer número de estágios, boosters e grupos de motores).
- TVC só no núcleo (teste); a tela de TVC entra com a eletrônica (fase 7).

## Fase 6: o que ficou para depois

- Editor visual por blocos é uma lista arrastável com formulários; não há desenho do
  mecanismo em diagrama.
- Animação 3D dos lançadores usa as trajetórias dos corpos (carrinho, ponta do braço, avião,
  balão) no globo, sem modelos 3D próprios.
- Avaliação de plugins por estrelas e biblioteca online exigem um servidor; hoje a biblioteca
  é local (importar, exportar, duplicar, link compartilhável).
- Monte Carlo de erros de liberação: fase 11.
- Canhão, catapulta e estilingue são guias retilíneas (a catapulta não gira um braço).

## Fase 7: o que ficou para depois

- Emulação de firmware só para ATmega328P (Uno, Nano, Pro Mini). Mega, ESP32, Teensy e
  STM32 estão na biblioteca com as verificações elétricas, mas o firmware deles só roda por
  hardware-in-the-loop.
- Dispositivos virtuais: BMP280, MPU-6050 (I2C) e GPS (NMEA na serial). BMP388, MS5611,
  BNO055, cartão SD (SPI) e rádio LoRa (SPI) aparecem nas verificações, mas não respondem na
  emulação (o firmware recebe NACK/sem resposta, e o ASTRA avisa).
- Hardware-in-the-loop pela Web Serial implementado com protocolo documentado
  (docs/HIL.md), mas **não testado com placa real** nesta versão.
- O editor de fiação é por listas (de pino, para pino), sem desenho do esquemático.
- A espera na rampa entra só no cálculo de autonomia; a emulação liga a placa 3 s antes da
  ignição.

## Fase 8: o que ficou para depois

- **Preset Zyrion:** não há dado público; a carga é criada pelo editor assim que a equipe
  informar massa, dimensões, CG e requisitos.
- **Regras:** IREC 2024 v1.4 transcrita nos itens que o documento define (payload, fórmula
  do apogeu, recuperação, bônus de Cube Unit, pódio). Requisitos técnicos do DTEG e o RCSM/LOM
  da LASC não foram recebidos: os presets avisam e o usuário acrescenta as regras.
- **Vibração aleatória** não é prevista (sem modelo confiável para foguetes pequenos): a
  carga é comparada ao nível de mão de obra do GEVS.
- **Pirochoque** da carga de ejeção e da separação não é estimado.
- Os bônus de lançamento na quarta/quinta-feira e as penalidades de conduta da IREC não
  dependem do voo e ficam fora da conta.

## Fase 9: o que ficou para depois

- Preços: o ASTRA não traz preços de materiais nem de peças (variam demais e mudariam
  sem aviso); o usuário informa preço por item ou R$/kg por material, e o custo das peças
  sem preço é estimado pela massa.
- Impostos de importação são uma porcentagem informada por item; o ASTRA não calcula as
  regras da Receita Federal.
- Cotação: botão busca a taxa do BCE pela API pública Frankfurter no navegador; não foi
  possível testá-la a partir do ambiente de desenvolvimento (rede bloqueada). A cotação
  manual funciona sempre.
- PDF: o relatório para patrocinadores abre pronto para imprimir e o navegador salva em
  PDF (não há gerador de PDF próprio).
- Notas fiscais: só o número; anexar o arquivo fica para quando houver armazenamento.
- Pareto: varia materiais de até duas peças (até 48 combinações); trocar motor e
  dimensões no explorador fica para o otimizador da fase 11.

## Fase 10: o que ficou para depois

- **Efemérides:** sem a tabela do JPL (rede bloqueada), as janelas usam órbitas circulares e
  coplanares com longitude média J2000 transcrita de memória. As janelas Terra→Marte de
  2020, 2022, 2024 e 2026 caem a menos de 75 dias das reais (teste). Efemérides DE440 e
  N-corpos ficam para quando a tabela/arquivo for fornecido.
- **Lambert** sem voltas múltiplas; sem gráfico porkchop (precisa das efemérides).
- **Assistência gravitacional** e transferências de baixo empuxo não implementadas.
- **Reentrada** planar sobre planeta sem rotação, aquecimento só convectivo de estagnação
  (sem radiação do gás de choque, relevante acima de ~10 km/s), proteção térmica pelo
  equilíbrio radiativo (sem ablação nem condução).
- **Pouso propulsivo** planar: curva de gravidade com ignição por bisseção (início
  suborbital) ou guiagem simples com portão (início orbital); sem dinâmica de atitude, sem
  limite mínimo de empuxo.
- **Pouso do 1º estágio do Falcon 9** ainda não liga à simulação da fase 5: o exemplo da
  ferramenta usa números informados.
- **Vista 3D do Sistema Solar:** a transferência é desenhada em 2D; o globo 3D cobre Terra.
