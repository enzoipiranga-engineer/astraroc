# Dados externos pendentes

Itens que dependem de dados que não puderam ser obtidos automaticamente. Nada aqui é
inventado: enquanto o dado não chegar, o recurso correspondente não é implementado.

| Item | Fase | Situação (15/11/2026) |
| --- | --- | --- |
| Payload do foguete Zyrion | 8 | Busca na web não achou nenhuma menção pública. O editor de carga útil está pronto (fase 8); falta a equipe informar massa, dimensões, CG e requisitos de g, vibração e temperatura para virar preset. |
| Regras da LASC (categorias, payload, pontuação) | 8 | Recebido o LASC General Rules (LGR), ed. 1, rev. 1, vigente desde 17/03/2026. Ele trata de organização de equipe, inscrição e prêmios, e diz que pontuação e critérios técnicos ficam nos manuais de cada desafio. **Falta o Rocket Challenge Standard Manual (RCSM)** (e o Launch Operations Manual, LOM, para regras de segurança de voo). |
| Regras da Spaceport America Cup / IREC | 8 | Recebido o Rules & Requirements Document **2024, v1.4** (vigente desde 04/03/2024). Pode haver versão mais nova; confirmar antes da competição. Falta o DTEG (Design, Test & Evaluation Guide), que traz os requisitos técnicos. |
| Chave do Cesium ion | 4 | O usuário aceitou criar; será pedida na Fase 4. |
| Repositório GitHub | já | Resolvido: código em enzoipiranga-engineer/astraroc. |
| Voo medido de garrafa PET (altímetro) | 2 | Nenhum dado medido disponível na sessão; a validação do foguete de água é só analítica. |
| Conferir tabela da atmosfera acima de 86 km (Vallado, Tabela 8-4) e dados do KNSB (Nakka) | 1 e 2 | Transcritos de memória; fontes não acessíveis nesta sessão. |
| Elementos dos planetas (Standish, tabela 1: a, e, L0, taxa de L) e atmosferas de Vênus e Titã | 10 | ssd.jpl.nasa.gov fora da rede permitida; valores transcritos de memória. Janelas de Marte conferem com as reais (±75 dias), mas convém conferir a tabela. |
| Curvas de motores pequenos (Estes etc.) | 2 | thrustcurve.org fora da rede permitida; só o Cesaroni M1670 está embutido. |

## Resumo do LASC General Rules (LGR), ed. 1, rev. 1 (março de 2026)

Útil para a fase 8 (regras de competição); nada aqui define física ou pontuação.

- Documentos em inglês (ORG 2.1.1). Hierarquia: LGR e EGM acima de LOM, SCSM, LCSM e
  RCSM (seção 2.2). "Shall/must" é obrigatório; "should" é recomendado.
- Equipe com no mínimo 4 membros; cada missão exige Mission Leader, Safety Manager,
  Operations Manager e Technology Manager, pessoas distintas (ORG 3.1.2, 3.1.6, 3.1.7).
- Responsável legal maior de 18 anos; menores precisam de autorização (ORG 3.1.4, 3.1.5).
- Rocketry Mentor recomendado, e na prática exigido para equipes novas e para propulsão
  híbrida ou líquida (ORG 3.2.1).
- Até 3 missões selecionadas por equipe, uma por categoria; exemplo de categorias citado:
  1 km AGL híbrido/líquido e 0,5 km AGL (ORG 4.1.2, 4.1.3).
- Mission IDs: foguetes 001 a 199, satélites 201 a 399, lander 401 a 600 (seção 4.1).
- Lançamento no Sugarcane Launch Range (SLR) após Launch Readiness Review com checklist
  e Flight Card (seções 1.1 e 3.5).
- Prêmios por categoria (1º e 2º lugar) e gerais; pontuação definida nos manuais de cada
  desafio (seção 5).

## Resumo do SA Cup IREC Rules & Requirements, 2024 v1.4

Para a fase 8. Fonte: `docs/regras/SA_Cup_IREC_Rules_2024_v1.4.pdf`.

- **Categorias (2.0):** apogeu-alvo de 10.000 ft ou 30.000 ft AGL, com propulsão COTS
  (sólido ou híbrido), SRAD sólido, ou SRAD híbrido/líquido: seis categorias.
- **Payload (2.3):** no mínimo 8,8 lb (≈ 3,99 kg); tolera pesagem até 8,4 lb por
  diferença de balança. Pode ser considerado na estabilidade; o foguete não precisa ser
  estável sem ele. Payload não funcional em "Payload Cube Units" de 100 × 100 mm por
  100, 200, 300 ou 400 mm (±2 mm). Sem chumbo/metais pesados em quantidade, sem animais
  vertebrados. Cada um dos cinco requisitos violado custa −100 pontos (2.7.1.6).
- **Altímetro oficial (2.5):** registro COTS de apogeu; GPS de rastreio obrigatório (2.4).
- **Pontuação (2.7.1), 1000 pontos:**
  - formulário e atualizações: 60 (15 cada, passa/não passa);
  - relatório técnico: 200 (correção 40, completude 20, análise 140);
  - projeto e construção: 240 (120 + 120);
  - desempenho de voo: 500, sendo **apogeu 350** e **recuperação 150** (passa/não passa:
    sem dano que impeça novo voo seguro).
  - **Fórmula do apogeu:** pontos = 350 − (350 / (0,3 × alvo)) × |alvo − apogeu|, só
    dentro de ±30% do alvo (fora disso, zero).
  - Bônus: +50 payload funcional no formato Cube Unit; +50 lançar na quarta, +25 na
    quinta. Penalidade: −20 por conduta insegura ou antidesportiva.
- **Elegibilidade ao pódio da categoria:** atingir pelo menos metade do apogeu-alvo.
- **Prêmio James Barrowman:** menor erro percentual entre apogeu previsto (declarado antes
  do voo) e o real. Exatamente o que o ASTRA precisa estimar bem, com incerteza.
