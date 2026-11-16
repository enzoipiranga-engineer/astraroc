# ASTRA para computador (Tauri 2)

O app de computador é a mesma interface web empacotada numa janela nativa. O núcleo WASM
roda localmente, sem servidor (o service worker do modo navegador não é usado). O globo 3D
ainda precisa de internet para imagens e terreno.

## Compilar

Linux (Ubuntu 24.04):

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libsoup-3.0-dev librsvg2-dev libayatana-appindicator3-dev
```

```bash
pnpm desktop
```

O binário fica em `apps/desktop/src-tauri/target/release/astra-desktop`. Para gerar
instaladores (.deb, AppImage, .msi, .dmg) use a CLI do Tauri
(`cargo install tauri-cli --version "^2" --locked` e `cargo tauri build` dentro de
`apps/desktop/src-tauri`).

## Estado

- Compilado e aberto no Linux (Ubuntu 24.04, Xvfb): a interface carrega e o núcleo WASM
  responde.
- Windows e macOS não foram testados. Instaladores não foram gerados nesta sessão.
- Fora da CI (a compilação leva alguns minutos e precisa das bibliotecas do sistema).
