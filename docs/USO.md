# Guia de uso

## Primeiro voo (1 minuto)

1. No topo, **abrir exemplo…** → "Competição 500 m (exemplo)".
2. Aperte **LANÇAR**. O globo 3D mostra o voo; os gráficos ficam embaixo. Clique num gráfico
   para levar o 3D até aquele instante.
3. No **mapa de recuperação** veja onde cada parte pousou e a que distância.

## Projetar

- **Peças:** clique no desenho ou na lista. Medidas, material e massa medida mudam tudo na
  hora (massa, CG, CP, margem, apogeu, custo). A barra de números mostra o efeito da última
  mudança (+/−).
- **Juntas:** cada peça precisa estar presa a outra. Sem junta, ela solta no voo — e o
  ASTRA simula a soltura.
- **Verificação física** (à direita): erros, avisos e sugestões em linguagem simples. Você
  pode lançar assim mesmo e ver a consequência.
- **Motor e trilho, local, vento:** carregue um .eng, escolha o local (busca, clique no mapa
  ou presets) e o vento (manual, CSV ou previsão real da Open-Meteo).

## Recursos avançados

| Onde | O que |
| --- | --- |
| Motor e lançador | Rampas, canhões, plataforma giratória, avião, balão, navio, plugins |
| Eletrônica e firmware | Monte placa, sensores, bateria, MOSFET e e-match; ligue os fios; carregue o .hex do Arduino (ou use o exemplo) e **VOAR COM FIRMWARE** |
| Carga útil e regras | Requisitos da carga e regras da competição; depois do voo, relatório aprovado/reprovado e placar |
| Finanças | Preços, frete, impostos, orçamento por subsistema, receitas, Pareto, planilha e relatório para patrocinadores |
| Análises | Monte Carlo (elipse de dispersão), sensibilidade, otimizador de apogeu, calibração com o CSV do altímetro |
| Lançador orbital | Falcon 9 e Starship até a órbita, com orçamento de Δv e custo por kg |
| Espaço | Manobras, Lambert, viagens interplanetárias, Lagrange, reentrada e pouso na Lua/Marte |

## Arquivos

- **Exportar / Importar:** projeto em JSON. **OpenRocket:** importa .ork (com avisos do que
  falta). **Link:** copia um endereço com o projeto dentro (nada vai para servidor).
- Depois do voo: KML (Google Earth), CSV de telemetria, relatório de voo (PDF), motor (.eng).
- **SI/US** alterna as unidades dos campos.

## Offline e computador

- No navegador, depois da primeira visita o ASTRA funciona sem internet (o globo 3D precisa
  de internet para imagens). Pelo menu do navegador dá para instalá-lo como aplicativo.
- App de computador: ver [DESKTOP.md](DESKTOP.md).
