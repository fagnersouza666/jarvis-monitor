# Origem e licenças de terceiros

Este documento identifica dependências e recursos. Ele **não escolhe uma
licença para o código próprio** do JARVIS Monitor, nem altera licenças de terceiros.

## Código e recursos do repositório

- Rust, TypeScript, HTML, CSS e scripts foram implementados para este projeto
  com assistência de IA. Não foi importado um template de aplicativo de terceiros.
- O ícone em `src-tauri/icons/` foi produzido com um fundo, um losango e traços
  geométricos. O SVG é a versão vetorial; os PNGs foram renderizados localmente
  com Pillow. Nenhuma fonte, fotografia, screenshot ou imagem de referência foi
  incorporada a esses arquivos.
- A interface usa fontes instaladas no sistema por nome/fallback; nenhum arquivo
  de fonte é distribuído.
- As imagens de referência visual de terceiros não são parte deste repositório.

## Dependências diretas

As licenças abaixo foram conferidas nos manifests das versões resolvidas:

| Componente | Versão conferida | Origem | Licença declarada |
| --- | --- | --- | --- |
| `tauri` | 2.12.4 | [tauri-apps/tauri](https://github.com/tauri-apps/tauri) | Apache-2.0 OR MIT |
| `tauri-build` | 2.7.3 | [tauri-apps/tauri](https://github.com/tauri-apps/tauri) | Apache-2.0 OR MIT |
| `serde_json` | 1.0.154 no app; 1.0.149 no lockfile isolado do coletor | [serde-rs/json](https://github.com/serde-rs/json) | MIT OR Apache-2.0 |
| `libc` | 0.2.191 no app; 0.2.183 no lockfile isolado do coletor | [rust-lang/libc](https://github.com/rust-lang/libc) | MIT OR Apache-2.0 |
| `@tauri-apps/cli` | 2.12.3 | [tauri-apps/tauri](https://github.com/tauri-apps/tauri) | Apache-2.0 OR MIT |
| TypeScript | 5.9.3 | [microsoft/TypeScript](https://github.com/microsoft/TypeScript) | Apache-2.0 |

Os lockfiles preservam as versões e checksums. Consulte o
[inventário das dependências Rust](docs/dependency-licenses.md) e os manifests
originais para as expressões SPDX das dependências transitivas. O inventário
inclui pacotes de build e de plataformas que o projeto não validou.

## Bibliotecas e ferramentas do sistema

O aplicativo Linux utiliza GTK3/WebKitGTK e bibliotecas transitivas fornecidas
pelo sistema. `systemctl` e `nvidia-smi` são chamados como ferramentas externas
quando disponíveis; seus binários e drivers não são distribuídos pelo projeto.
Rust/Cargo, Node/npm e Pillow são ferramentas de construção, não código copiado
para a árvore. As licenças desses componentes continuam sendo as dos respectivos
fornecedores e pacotes da distribuição.

Esta publicação contém código-fonte e manifests, sem dependências vendorizadas,
cabeçalhos do sistema, drivers, executáveis ou instaladores. Antes de distribuir
binários ou incorporar dependências ao próprio pacote, confira os textos de
licença, avisos e obrigações aplicáveis às versões efetivamente incluídas. Não
aplique uma eventual licença futura do projeto às dependências de terceiros.
