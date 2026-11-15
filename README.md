# ASTRA

Simulador de foguetes com física real, da garrafa PET ao Starship. Em construção por fases;
veja [docs/PLANO.md](docs/PLANO.md) para o estado atual.

> Ferramenta de estudo e projeto. Não substitui análise de segurança, autorização de espaço
> aéreo nem as normas de lançamento locais.

## Requisitos

- Rust estável com o alvo `wasm32-unknown-unknown`
- `wasm-bindgen-cli` 0.2.130 (`cargo install wasm-bindgen-cli --version 0.2.130 --locked`)
- Node 22 e pnpm 10

## Comandos

| Comando | O que faz |
| --- | --- |
| `pnpm install` | Instala as dependências JS |
| `pnpm dev` | Compila o núcleo para WASM e abre o app |
| `pnpm test` | Testes do núcleo (Rust) e da ponte JS |
| `pnpm typecheck` | Checagem de tipos do TypeScript |
| `pnpm build` | Build de produção do app web |

## Documentos

- [Arquitetura](docs/ARQUITETURA.md)
- [Plano por fases](docs/PLANO.md)
- [Modelos físicos e limitações](docs/MODELOS.md)
- [Validação](docs/VALIDACAO.md)
