<p align="center">
  <img src="src-tauri/icons/icon.svg" width="88" height="88" alt="Símbolo do JARVIS Monitor">
</p>

# JARVIS Monitor

Monitor desktop de hardware para Linux, com visual azul-marinho e ciano, dois
painéis e métricas reais da máquina. O coletor em **Rust** e a interface
**HTML/CSS/TypeScript** são integrados com **Tauri 2** em uma única aplicação.

Abre em janela própria pelo menu de aplicativos. Não precisa iniciar Python,
servidor HTTP ou navegador. O aplicativo é **somente leitura** e não abre porta TCP.

**Plataforma validada:** Pop!_OS 24.04 LTS, arquitetura amd64. Outras distribuições,
Windows e macOS não foram validados. Este repositório publica o código-fonte;
não há release ou download de binário publicado nesta etapa.

## Dois painéis

| Painel | O que apresenta |
| --- | --- |
| **Visão geral** | CPU e RAM, relógio circular com hora local, GPU/VRAM NVIDIA, serviços observados e avisos com critérios explícitos. |
| **Hardware** | Carga da CPU por thread, RAM e swap, GPU/VRAM, volumes locais, temperaturas e os dez processos com maior memória residente. |

São as duas únicas abas. Não há botões de reiniciar serviços, encerrar processos,
limpar memória ou executar comandos no sistema. Animações respeitam
`prefers-reduced-motion`; a janela também oferece uma opção para reduzi-las.

## Métricas reais e estados honestos

- **CPU:** diferenças entre contadores de `/proc/stat`, com amostra inicial antes
  de calcular o uso. Modelo, núcleos e frequência vêm de informações do kernel.
- **RAM:** uso calculado por total menos memória disponível. A memória disponível
  inclui a parte recuperável do cache do Linux.
- **Swap:** medida separadamente. Swap não configurada é diferente de sensor
  indisponível e de swap configurada com uso igual a zero.
- **GPU NVIDIA:** utilização, VRAM e temperatura via `nvidia-smi`, quando o
  dispositivo, o driver e a ferramenta estão acessíveis. Outras marcas de GPU
  ainda não têm coletor específico.
- **Discos:** capacidade e ocupação dos sistemas de arquivos locais montados.
  Volumes Snap/loop, remotos e FUSE são omitidos; dispositivos repetidos aparecem
  uma vez. Não é um teste SMART ou de integridade do disco.
- **Temperaturas:** somente sensores que o kernel disponibiliza em sysfs. Nem
  todo equipamento expõe todos os sensores.
- **Processos:** apenas PID, nome, RSS e percentual da RAM. RSS pode incluir
  páginas compartilhadas; não equivale à memória exclusiva de cada processo.
- **Serviços:** Docker, llama-server e Hermes por padrão, observados por nomes
  exatos de processos e unidades systemd de sistema/usuário. Os estados são
  **ativo**, **parado**, **falhou**, **não instalado nos nomes configurados** e
  **desconhecido**. Processo ativo não comprova saúde funcional da aplicação.
- **Uptime:** tempo ligado informado pelo kernel.

Quando uma leitura não existe, falha ou não tem permissão, a interface mostra
**indisponível/N/D**. Não substitui ausência de sensor por zero. Uma falha de
RAM, swap ou outro subsistema não deve impedir as demais leituras. Se a coleta
ficar antiga ou o IPC falhar, a janela identifica os valores como antigos.

## Instalação

### A partir de um `.deb` construído localmente

Depois de [compilar o projeto](#compilar), o pacote fica em:

```text
release/jarvis-monitor_1.0.0_amd64.deb
release/SHA256SUMS
```

Verifique o checksum gerado e escolha **uma** modalidade de instalação:

```bash
(cd release && sha256sum -c SHA256SUMS)
```

**Para o usuário atual, sem root:**

```bash
bash scripts/install-user.sh
```

O script extrai o mesmo `.deb`, instala o executável em
`~/.local/lib/jarvis-monitor/1.0.0/`, cria `~/.local/bin/jarvis-monitor` e registra
o atalho **JARVIS Monitor** no menu. Ele interrompe se um destino já existir,
para preservar a instalação anterior. Não instala bibliotecas do sistema.

**Pelo gerenciador de pacotes, para o sistema:**

```bash
sudo apt install ./release/jarvis-monitor_1.0.0_amd64.deb
```

Essa modalidade pede autenticação de administrador e resolve as dependências
declaradas no pacote. A instalação pelo gerenciador gráfico de pacotes também
é possível. Evite manter as duas modalidades instaladas ao mesmo tempo.

### Abrir e fechar

Abra **JARVIS Monitor** no menu, ou execute o comando da modalidade instalada:

```bash
# Instalação por usuário
~/.local/bin/jarvis-monitor

# Instalação de sistema
jarvis-monitor
```

Para parar, **feche a janela**. Não há tray, daemon, serviço de inicialização,
updater ou autostart configurado pelo aplicativo.

### Desinstalar

Na modalidade de sistema, use `sudo apt remove jarvis-monitor`.
Na modalidade por usuário, feche a janela e remova apenas os arquivos abaixo:

```text
~/.local/lib/jarvis-monitor/1.0.0/
~/.local/bin/jarvis-monitor
~/.local/share/applications/local.jarvis.Monitor.desktop
~/.local/share/icons/hicolor/128x128/apps/local.jarvis.monitor.png
```

As preferências de movimento da interface podem permanecer no armazenamento
local do WebKit associado ao aplicativo. O programa não remove dados de outros apps.

## Configuração

### Intervalo de atualização

O padrão é **2 segundos**. Feche a janela anterior e abra com um intervalo entre
1 e 60 segundos:

```bash
jarvis-monitor --interval 1
# ou ~/.local/bin/jarvis-monitor --interval 1
```

A coleta acontece em uma thread compartilhada; abrir uma aba não inicia outro
coletor. Consultas a unidades systemd têm cache de pelo menos 10 segundos; nomes
dos processos são observados em cada amostra. A idade da consulta aparece na tela.

### Serviços observados

A lista está em [`services.json`](services.json) e é **embutida durante a
compilação**. A versão instalada não lê um arquivo externo de configuração.
Para adaptar nomes de unidades ou processos, edite o JSON e recompile:

```json
{
  "name": "Inferência local",
  "units": ["inference.service"],
  "process_names": ["llama-server"],
  "executables": ["llama-server"]
}
```

Esse objeto é um item da lista. Use nomes exatos e unidades terminadas em
`.service`. `executables` verifica existência/metadados no PATH; não executa
esses programas. Não coloque tokens, argumentos, URLs privadas ou segredos nesse
arquivo. O coletor não examina linhas de comando para tentar adivinhar serviços.

### Avisos do diagnóstico

Os limites por amostra são CPU ou RAM **≥90%**, volume **≥90%** e temperatura
**≥85°C**. As regras e a cobertura limitada são exibidas na interface. Ausência
de aviso não certifica a saúde do hardware: não há teste de memória, SMART,
diagnóstico elétrico ou verificação funcional dos serviços.

## Arquitetura

```text
Kernel Linux: /proc e sysfs      nvidia-smi / systemctl (consultas limitadas)
                \                         /
                 coletor Rust (telemetry/)
                           |
                 última amostra em memória
                           |
                  IPC Tauri: metrics
                           |
                  HTML + CSS + TypeScript
```

| Diretório/arquivo | Responsabilidade |
| --- | --- |
| `telemetry/` | Coleta Linux, normalização de valores ausentes e testes. |
| `src-tauri/` | Janela, amostragem compartilhada e único comando IPC de leitura. |
| `ui/` | Duas telas, relógio, indicadores, acessibilidade e estados de coleta. |
| `services.json` | Nomes de serviços embutidos na compilação. |
| `scripts/` | Build, normalização do `.deb` e instalação por usuário. |

Consultas a subprocessos têm prazo e saída limitada. Somente um filho criado
pelo próprio coletor pode ser encerrado por timeout; a espera posterior ocorre
em um reaper separado, com limite de comandos/reapers pendentes. Não existe
comando IPC para executar shell, escrever arquivos ou controlar processos.

## Requisitos

**Para executar o pacote:** Linux amd64 compatível com o build, GTK3 e
WebKitGTK 4.1. `nvidia-smi` e systemd são opcionais para suas respectivas
leituras; não são instalados nem configurados automaticamente. O pacote não é
um executável estaticamente autossuficiente. Rust, Node e Python não são
necessários para abrir o aplicativo instalado.

**Para compilar no Pop!_OS 24.04:** Rust/Cargo, Node/npm e as bibliotecas de
desenvolvimento do Tauri. A versão inicial foi compilada com Rust 1.99.1 e Node
26; não foi estabelecida uma versão mínima de toolchain compatível.

Após revisar as alterações propostas pelo apt, as dependências de build podem
ser instaladas pelos repositórios oficiais:

```bash
sudo apt install build-essential pkg-config libgtk-3-dev libwebkit2gtk-4.1-dev \
  libssl-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev \
  patchelf desktop-file-utils
```

Consulte os [pré-requisitos oficiais do Tauri](https://v2.tauri.app/start/prerequisites/)
para os detalhes atualizados. Instalar o app não exige instalar todo o ambiente de build.

## Compilar

```bash
git clone https://github.com/fagnersouza666/jarvis-monitor.git
cd jarvis-monitor
npm ci --ignore-scripts
bash scripts/build-release.sh
```

O script limita a compilação a dois jobs por padrão; ajuste `CARGO_BUILD_JOBS`
se desejar. O executável fica em `target/release/jarvis-monitor`; o `.deb` e
checksum são normalizados em `release/`. Os lockfiles npm e Cargo são mantidos
no repositório. Na primeira compilação, o Cargo precisa baixar suas dependências.
Depois de preencher os caches, é possível usar `CARGO_NET_OFFLINE=true`.

```bash
# Somente a interface
npm run build:ui

# Executável compilado, sem instalar
./target/release/jarvis-monitor
```

## Testes e verificações

```bash
npm run check:ui
cargo test --locked --manifest-path telemetry/Cargo.toml
cargo clippy --locked --manifest-path telemetry/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path telemetry/Cargo.toml -- --check
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
for script in scripts/*.sh; do bash -n "$script"; done
```

Os 15 testes Rust cobrem delta e reset dos contadores de CPU, RAM/swap
independentes, NVIDIA com valores ausentes, aliases e estados de serviços,
processos mínimos, discos locais, sensores, ausência de subsistemas, prazos e
limites de subprocessos. Há fixtures sintéticas nos testes; elas não alimentam
a aplicação em execução.

A versão inicial também foi verificada em janela Tauri release no Pop!_OS,
com atualização de métricas reais, navegação e redução de movimento. Dados
ausentes e falha de IPC foram exercitados no frontend em uma sessão de QA
isolada. Logs e capturas reais dessa máquina não integram este repositório.

Para uma leitura manual de desenvolvimento, `--snapshot` ou o exemplo abaixo
imprime uma amostra real no terminal. Revise e sanitize a saída antes de anexá-la
a uma issue, pois ela contém nomes e PIDs de processos da sua máquina:

```bash
cargo run --locked --manifest-path telemetry/Cargo.toml --example snapshot
```

## Privacidade e limites de acesso

O monitor não transmite telemetria, não usa fontes ou scripts remotos e não
configura credenciais, firewall ou acesso persistente. Não solicita root para
coletar sensores inacessíveis. Não expõe command lines, ambiente, usuários dos
processos ou conteúdo de arquivos pessoais no JSON. Executa com as permissões
normais da conta; não é uma barreira de isolamento entre aplicativos dessa conta.

Para relatar problemas, use exemplos sintéticos e remova segredos, caminhos,
nomes privados e detalhes desnecessários da máquina. Consulte [PRIVACY.md](PRIVACY.md).

## Origem dos recursos e licença

Código e interface foram implementados para este projeto com assistência de IA.
O ícone distribuído nesta árvore foi construído com formas geométricas próprias,
sem fonte ou imagem externa incorporada. Não são publicadas imagens de referência
de terceiros, capturas reais da máquina ou um protótipo anterior em Python.

**A licença do código próprio ainda não foi definida pelo titular.** Este
repositório não acrescenta uma licença permissiva ao projeto. As dependências
continuam sob suas licenças originais; consulte [THIRD_PARTY.md](THIRD_PARTY.md)
para a origem e o inventário. A disponibilidade pública do repositório não deve
ser apresentada como uma escolha de licença open source que ainda não foi feita.
