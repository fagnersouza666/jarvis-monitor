# Privacidade

A aplicação coleta somente informações locais necessárias aos dois painéis.
O código não contém endpoint externo de telemetria nem servidor HTTP de métricas.
O frontend usa o comando IPC `metrics` para ler uma cópia da última amostra.

## Dados utilizados

- Contadores CPU, RAM/swap e uptime de `/proc`.
- Nome e RSS dos processos via `comm` e `statm`, além de PID e percentual de RAM.
- Capacidade dos sistemas de arquivos locais montados; caminhos pessoais de
  montagem são substituídos por rótulos mínimos de dispositivo.
- Sensores térmicos publicados pelo kernel.
- Campos selecionados do `nvidia-smi` e propriedades selecionadas das unidades systemd.
- Existência/metadados dos executáveis configurados, nos diretórios do PATH.

O programa não retorna linhas de comando, variáveis de ambiente, nomes de
usuários dos processos ou conteúdo de arquivos pessoais. O PATH serve apenas
para localizar executáveis configurados; seu valor não integra o snapshot.

## Armazenamento

A última amostra fica em memória. O coletor não cria histórico em disco.
A opção de reduzir movimento fica no armazenamento local do WebKit da aplicação.
`--snapshot` e o exemplo Rust de diagnóstico imprimem informações reais se forem
invocados explicitamente pelo desenvolvedor; não publique essa saída sem revisão.

## Ao abrir issues ou compartilhar imagens

Use fixtures sintéticas, ou sanitize os dados. Remova nomes de processos privados,
PIDs desnecessários, identificadores, caminhos, nomes de volumes e qualquer segredo.
Não envie tokens, ambiente, arquivos pessoais ou command lines para diagnosticar
um sensor. Um sensor sem suporte ou permissão deve permanecer indisponível.

## Escopo

Sem privilégios elevados, autostart, updater, firewall ou ações de controle sobre
serviços/processos. O programa roda com os direitos usuais da conta e não isola
os demais aplicativos da mesma conta. Um problema de kernel/driver pode limitar
a coleta; dados antigos ficam sinalizados na interface.
