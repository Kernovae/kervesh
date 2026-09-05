# Kernovae SSH Manager

## Product Requirements Draft

**Status:** Ideacao / requisitos iniciais  
**Organizacao:** Kernovae  
**Plataformas alvo:** Windows e Linux  
**Implementacao principal:** Rust  
**Nome do produto:** a definir

---

## 1. Visao do produto

O projeto sera um gerenciador desktop de conexoes SSH e administracao remota inspirado na produtividade de ferramentas como MobaXterm, mas construido com uma filosofia diferente: software open source, nativo, local-first, leve e multiplataforma.

O aplicativo deve unir, em uma unica interface, terminal SSH, gerenciamento de hosts, SFTP visual, transferencias de arquivos, edicao remota, monitoramento do host conectado e recursos de administracao de conexao. O usuario nao deve precisar criar conta, depender de um servidor da Kernovae ou instalar uma camada de compatibilidade para utilizar o produto.

A aplicacao deve ser capaz de rodar nativamente em Windows e em diferentes distribuicoes Linux, com builds proprios para cada plataforma suportada. A meta e evitar runtimes pesados ou arquiteturas baseadas em navegador embarcado.

### Proposta de valor

> Um workspace SSH/SFTP open source, nativo, local-first e leve, com monitoramento remoto integrado e sem dependencia obrigatoria de nuvem.

### Principios do produto

- Rust-first.
- Sem Electron, Chromium ou Node.js embarcado como runtime da interface.
- Sem conta obrigatoria.
- Sem backend em nuvem obrigatorio.
- Local-first e funcional offline, exceto pela propria conectividade necessaria para acessar hosts remotos.
- Sem agente obrigatorio no servidor remoto para o monitoramento basico.
- Comportamento consistente entre Windows e Linux.
- Segredos nunca armazenados em texto puro.
- Configuracoes exportaveis e documentadas.
- Baixo uso de RAM, CPU e disco como requisito mensuravel.
- Arquitetura modular para permitir evolucao sem transformar o projeto em um monolito.

---

## 2. Escopo funcional de alto nivel

O produto devera ser composto inicialmente pelos seguintes dominios:

1. Gerenciamento de conexoes SSH.
2. Terminal remoto completo.
3. Navegador SFTP integrado.
4. Gerenciador de transferencias.
5. Monitoramento do dispositivo conectado via SSH.
6. Gerenciamento seguro de credenciais e chaves.
7. Organizacao de hosts, grupos e favoritos.
8. Persistencia local e exportacao de configuracoes.
9. Suporte nativo a Windows e Linux.
10. Recursos avancados de SSH, como bastion/jump host e port forwarding, em fases posteriores.

---

## 3. Experiencia de interface

A interface deve priorizar densidade de informacao e produtividade, mantendo um visual limpo e profissional.

Exemplo conceitual:

```text
+--------------------------------------------------------------------------------+
| Product                                                   Settings      _ [] X |
+----------------+---------------------------------------------------------------+
| CONNECTIONS    | prod-01 | homelab | database | +                              |
|                +---------------------------------------------------------------+
| Production     |                                                               |
|  * prod-01     | root@prod-01:~$                                              |
|  o prod-02     |                                                               |
|                |                                                               |
| Homelab        |                         TERMINAL                              |
|  * ubuntu      |                                                               |
|  o fedora      |                                                               |
|                |                                                               |
|----------------|                                                               |
| SFTP           |                                                               |
| /var/www       |                                                               |
|  [D] app       |                                                               |
|  [D] logs      |                                                               |
|  [F] nginx.conf|                                                               |
|                +---------------------------------------------------------------+
| Up Dn + Refresh| CPU 24% | RAM 61% | / 42% | /data 81% | Net 2.8 / 0.7 MB/s |
+----------------+---------------------------------------------------------------+
```

### 3.1 Layout esperado

- Sidebar de conexoes.
- Organizacao por grupos/pastas.
- Tabs para sessoes simultaneas.
- Painel SFTP lateral associado a sessao atual.
- Terminal como area principal.
- Barra compacta de monitoramento remoto.
- Inspector detalhado de sistema acessivel sob demanda.
- Painel de transferencias desacoplado da navegacao SFTP.
- Sidebars recolhiveis para maximizar espaco do terminal.

---

## 4. Gerenciamento de conexoes SSH

| ID | Requisito |
|---|---|
| SSH-001 | Criar uma conexao SSH. |
| SSH-002 | Editar uma conexao existente. |
| SSH-003 | Excluir uma conexao. |
| SSH-004 | Duplicar uma conexao. |
| SSH-005 | Testar conectividade antes de salvar. |
| SSH-006 | Definir nome amigavel da conexao. |
| SSH-007 | Configurar hostname ou endereco IP. |
| SSH-008 | Configurar porta SSH. |
| SSH-009 | Configurar usuario. |
| SSH-010 | Autenticacao por senha. |
| SSH-011 | Autenticacao por chave privada. |
| SSH-012 | Suporte a chave protegida por passphrase. |
| SSH-013 | Integracao com SSH Agent quando disponivel. |
| SSH-014 | Salvar sessao localmente. |
| SSH-015 | Organizar conexoes em grupos/pastas. |
| SSH-016 | Adicionar tags. |
| SSH-017 | Marcar favoritos. |
| SSH-018 | Pesquisa instantanea de hosts. |
| SSH-019 | Historico de conexoes recentes. |
| SSH-020 | Keepalive configuravel. |
| SSH-021 | Timeout configuravel. |
| SSH-022 | Reconexao automatica opcional. |
| SSH-023 | Verificacao de host key. |
| SSH-024 | Gerenciamento de known_hosts. |
| SSH-025 | Alerta de mudanca de fingerprint. |
| SSH-026 | Suporte IPv4. |
| SSH-027 | Suporte IPv6. |
| SSH-028 | Importar configuracoes de OpenSSH quando possivel. |
| SSH-029 | Exportar conexoes em formato documentado. |
| SSH-030 | Jump host / bastion host. |
| SSH-031 | ProxyCommand ou mecanismo equivalente. |

### Decisao arquitetural importante

A aplicacao nao deve depender obrigatoriamente do executavel `ssh` instalado no sistema operacional. O core deve possuir uma implementacao SSH embutida ou uma biblioteca Rust que permita comportamento consistente entre plataformas.

---

## 5. Terminal remoto

O terminal deve ser um emulador de terminal real, e nao apenas uma caixa que imprime stdout.

### Requisitos principais

| ID | Requisito |
|---|---|
| TERM-001 | Suporte a ANSI escape sequences. |
| TERM-002 | Compatibilidade com comportamento esperado de terminais VT/xterm modernos. |
| TERM-003 | UTF-8 e Unicode. |
| TERM-004 | 16, 256 cores e true color. |
| TERM-005 | Scrollback configuravel. |
| TERM-006 | Selecao e copia de texto. |
| TERM-007 | Colagem com bracketed paste. |
| TERM-008 | Redimensionamento de PTY. |
| TERM-009 | Suporte a alternate screen. |
| TERM-010 | Eventos de mouse para programas remotos que os utilizem. |
| TERM-011 | Atalhos configuraveis. |
| TERM-012 | Fonte e tamanho configuraveis. |
| TERM-013 | Temas claro e escuro. |
| TERM-014 | Multiplas tabs independentes. |
| TERM-015 | Abertura simultanea de diversas sessoes sem bloquear a UI. |

Ferramentas como `vim`, `nvim`, `nano`, `htop`, `btop`, `less`, `tmux` e `mc` devem funcionar corretamente dentro do terminal.

---

## 6. SFTP integrado

O SFTP e parte central da experiencia e deve estar ligado diretamente a sessao SSH ativa.

| ID | Requisito |
|---|---|
| SFTP-001 | Listar arquivos e diretorios. |
| SFTP-002 | Navegar entre diretorios. |
| SFTP-003 | Voltar ao diretorio anterior. |
| SFTP-004 | Subir um nivel. |
| SFTP-005 | Atualizar/refresh da pasta atual. |
| SFTP-006 | Criar arquivo. |
| SFTP-007 | Criar diretorio. |
| SFTP-008 | Renomear arquivo ou diretorio. |
| SFTP-009 | Excluir arquivo. |
| SFTP-010 | Excluir diretorio. |
| SFTP-011 | Download de arquivo. |
| SFTP-012 | Download recursivo de diretorio. |
| SFTP-013 | Upload de arquivo. |
| SFTP-014 | Upload recursivo de diretorio. |
| SFTP-015 | Drag and drop para upload. |
| SFTP-016 | Confirmacao de sobrescrita. |
| SFTP-017 | Exibir tamanho. |
| SFTP-018 | Exibir data de modificacao. |
| SFTP-019 | Exibir owner e group quando disponivel. |
| SFTP-020 | Exibir e alterar permissoes. |
| SFTP-021 | Ordenar por nome, tamanho, tipo e data. |
| SFTP-022 | Pesquisar/filtrar itens. |
| SFTP-023 | Copiar caminho remoto. |
| SFTP-024 | Abrir arquivo remoto em editor integrado. |
| SFTP-025 | Mostrar arquivos ocultos de forma configuravel. |
| SFTP-026 | Atualizar a arvore sem reiniciar a sessao. |

---

## 7. Gerenciador de transferencias

Transferencias nao devem bloquear o terminal nem a interface SFTP.

| ID | Requisito |
|---|---|
| XFER-001 | Fila de transferencias. |
| XFER-002 | Upload e download assincronos. |
| XFER-003 | Indicador de progresso por item. |
| XFER-004 | Velocidade de transferencia. |
| XFER-005 | Bytes transferidos e total. |
| XFER-006 | Cancelamento. |
| XFER-007 | Retry. |
| XFER-008 | Historico da sessao. |
| XFER-009 | Transferencia de arquivos grandes por streaming. |
| XFER-010 | Nao carregar o arquivo inteiro na RAM. |

---

## 8. Editor remoto

O usuario deve poder abrir um arquivo pelo painel SFTP, edita-lo e salva-lo novamente no servidor sem executar manualmente download e upload.

Fluxo esperado:

```text
SFTP -> Open/Edit -> buffer local temporario -> Save -> upload SFTP -> arquivo remoto
```

### Requisitos

- Editor de texto leve.
- Indicacao de arquivo modificado.
- Confirmacao ao fechar com alteracoes nao salvas.
- Deteccao basica de alteracao concorrente no arquivo remoto quando possivel.
- Salvamento atomico quando suportado.
- Possibilidade futura de integrar editor externo local.

---

## 9. Monitoramento remoto nativo

O monitoramento do host conectado e requisito de primeira classe. A experiencia deve ser semelhante a uma ferramenta de administracao remota: o usuario conecta via SSH e imediatamente visualiza o estado basico da maquina.

### Principio central

> O monitoramento basico nao deve exigir a instalacao de um agente Kernovae no host remoto.

A coleta devera utilizar a propria conexao SSH e canais independentes do PTY interativo.

### Arquitetura conceitual

```text
                    SSH transport
                         |
       +-----------------+------------------+
       |                 |                  |
       v                 v                  v
 Terminal channel    SFTP channel      Monitor channels
      remote PTY                          exec/read
```

Com isso, comandos de coleta nunca aparecem no terminal do usuario.

### 9.1 Metricas obrigatorias

| ID | Requisito |
|---|---|
| MON-001 | Monitoramento sem agente obrigatorio. |
| MON-002 | Reutilizar a conexao SSH existente. |
| MON-003 | Utilizar canal independente do terminal. |
| MON-004 | CPU total. |
| MON-005 | CPU por core quando suportado. |
| MON-006 | Load average. |
| MON-007 | Memoria total. |
| MON-008 | Memoria usada e disponivel. |
| MON-009 | Cache/buffers quando aplicavel. |
| MON-010 | Swap total e utilizada. |
| MON-011 | Filesystems. |
| MON-012 | Discos e particoes. |
| MON-013 | Mountpoints. |
| MON-014 | Espaco usado/livre. |
| MON-015 | Uso de inodes quando suportado. |
| MON-016 | Interfaces de rede. |
| MON-017 | Taxa de RX/download por interface. |
| MON-018 | Taxa de TX/upload por interface. |
| MON-019 | Uptime. |
| MON-020 | Hostname. |
| MON-021 | Sistema operacional. |
| MON-022 | Kernel. |
| MON-023 | Arquitetura da maquina. |
| MON-024 | Modelo de CPU quando disponivel. |
| MON-025 | Numero de CPUs/cores. |
| MON-026 | File descriptors quando suportado. |
| MON-027 | Quantidade de processos. |
| MON-028 | Barra compacta abaixo do terminal. |
| MON-029 | Inspector detalhado. |
| MON-030 | Intervalo de atualizacao configuravel. |
| MON-031 | Metricas selecionaveis. |
| MON-032 | Pausar monitoramento. |
| MON-033 | Reativar monitoramento apos reconexao. |
| MON-034 | Deteccao automatica de capacidades do host. |
| MON-035 | Thresholds locais de aviso. |

### 9.2 Coleta Linux

Para Linux, o coletor deve preferir interfaces de sistema disponiveis nativamente, reduzindo dependencia de ferramentas externas.

Possiveis fontes:

- `/proc/stat` para CPU.
- `/proc/meminfo` para memoria e swap.
- `/proc/loadavg` para load average.
- `/proc/net/dev` para trafego por interface.
- `/proc/self/mountinfo` ou `/proc/mounts` para mountpoints.
- `/proc/sys/fs/file-nr` para file descriptors, quando aplicavel.
- `df` como fallback para espaco de filesystem.
- comandos como `uname` apenas quando necessarios para completar metadados.

A arquitetura de monitoramento nao deve assumir que todo host SSH e Ubuntu. O sistema deve detectar o sistema remoto e suas capacidades.

### 9.3 Capability probe

Ao estabelecer a conexao:

```text
SSH connected
     |
     v
Capability probe
     |
     +-- OS / uname
     +-- /proc available?
     +-- SFTP capabilities
     +-- filesystem/stat support
     +-- available fallback commands
     |
     v
RemoteCapabilities
```

O restante da aplicacao utiliza `RemoteCapabilities` e nao espalha verificacoes especificas de SO pela UI.

### 9.4 Frequencias de coleta sugeridas

| Metrica | Intervalo inicial sugerido |
|---|---:|
| CPU | 2 s |
| RAM/Swap | 2 s |
| Rede | 2 s |
| Load | 5 s |
| Filesystem usage | 10 s |
| Mountpoints | 30 s |
| Host/OS/CPU metadata | uma vez por conexao |

O scheduler deve permitir configuracao e evitar sobrecarga desnecessaria no servidor remoto.

### 9.5 Inspector detalhado

Exemplo conceitual:

```text
SYSTEM
Hostname       prod-db-01
OS             Linux
Kernel         6.x
Architecture   x86_64
Uptime         43d 12h

CPU
Total          34%
Cores          8
Load           1.22 0.91 0.72

MEMORY
Used           9.8 GB / 16 GB
Available      6.2 GB
Swap           0.4 GB / 4 GB

STORAGE
/              51%
/home          36%
/data          80%
/backup        90% WARNING

NETWORK
eth0           RX 18.3 MB/s | TX 3.4 MB/s
tailscale0     RX 240 KB/s  | TX 18 KB/s
```

---

## 10. Processos remotos - fase posterior

Um modulo posterior pode fornecer visualizacao de processos:

- PID.
- Usuario.
- CPU.
- Memoria.
- Comando.
- Pesquisa e ordenacao.
- Envio de SIGTERM.
- Envio de SIGKILL com confirmacao explicita.
- Copiar PID/comando.

Acoes destrutivas devem possuir confirmacao e controles claros.

---

## 11. Port forwarding e conectividade avancada

### Recursos previstos

- Local port forwarding.
- Remote port forwarding.
- Dynamic forwarding / SOCKS.
- Jump host / bastion.
- Encadeamento de conexoes quando tecnicamente justificavel.
- Configuracoes persistentes de tunel.
- Indicador de tunel ativo.

Exemplo:

```text
localhost:5433 -> SSH host -> remote localhost:5432
```

---

## 12. Seguranca e credenciais

Seguranca nao deve ser adicionada apenas no final do projeto.

### Requisitos

| ID | Requisito |
|---|---|
| SEC-001 | Senhas nunca armazenadas em texto puro. |
| SEC-002 | Passphrases nunca armazenadas em texto puro. |
| SEC-003 | Preferir credential store nativo do SO. |
| SEC-004 | Suporte a chave SSH armazenada em arquivo local. |
| SEC-005 | Host key checking habilitado por padrao. |
| SEC-006 | Alertar mudancas de fingerprint. |
| SEC-007 | Logs nao devem registrar segredos. |
| SEC-008 | Clipboard de senha deve ser evitado ou tratado cuidadosamente. |
| SEC-009 | Configuracoes e segredos devem ser armazenados separadamente. |
| SEC-010 | Exportacao nao inclui segredos por padrao. |
| SEC-011 | Nenhuma telemetria obrigatoria. |
| SEC-012 | Nenhuma dependencia de login Kernovae. |

Modelo conceitual:

```text
Connection configuration
  host
  user
  port
  credential_id
          |
          v
OS credential store / encrypted local vault
```

---

## 13. Persistencia local

O estado da aplicacao pode ser separado em dois grupos.

### Dados de configuracao

- Hosts.
- Grupos.
- Tags.
- Favoritos.
- Preferencias.
- Layouts.
- Historico.
- Configuracoes de monitoramento.

### Segredos

- Senhas.
- Passphrases.
- Tokens futuros.
- Material sensivel que nao deva aparecer em arquivos de configuracao.

Uma opcao arquitetural inicial e usar SQLite para estado interno e oferecer import/export em TOML ou JSON documentado.

---

## 14. Arquitetura proposta

```text
                         APPLICATION
                             |
        +--------------------+--------------------+
        |                                         |
        v                                         v
     UI Layer                              Application Core
        |                                         |
        +------------- Commands / Events ---------+
        |
        +-- Sessions UI
        +-- Terminal UI
        +-- SFTP UI
        +-- Monitor UI
        +-- Transfer UI
                             |
                             v
                       Domain services
                             |
        +--------------------+--------------------+
        |            |             |              |
        v            v             v              v
       SSH          SFTP         Monitor        Storage
        |            |             |              |
        +------------+-------------+--------------+
                             |
                             v
                       Platform layer
                             |
                    +--------+--------+
                    |                 |
                 Windows            Linux
```

### Objetivos dessa separacao

- UI nao conhece detalhes de protocolo SSH.
- Monitoramento nao conhece detalhes de renderizacao.
- Storage nao conhece widgets.
- Diferencas de plataforma ficam isoladas.
- Testes unitarios podem simular dominios sem abrir janelas.
- Componentes podem futuramente ser reutilizados como crates separados.

---

## 15. Estrutura sugerida de Cargo Workspace

```text
project/
|
+-- Cargo.toml
+-- crates/
|   +-- app/
|   +-- core/
|   +-- ui/
|   +-- terminal/
|   +-- ssh/
|   +-- sftp/
|   +-- monitor/
|   +-- transfer/
|   +-- storage/
|   +-- secrets/
|   +-- platform/
|
+-- assets/
+-- docs/
+-- packaging/
|   +-- windows/
|   +-- linux/
+-- tests/
```

Nomes dos crates poderao receber prefixo do produto apos a escolha definitiva do nome.

---

## 16. Stack tecnica candidata

Esta secao representa candidatos para spikes tecnicos, nao decisoes irreversiveis.

| Necessidade | Candidato inicial |
|---|---|
| Linguagem | Rust |
| Runtime async | Tokio |
| SSH | Russh ou biblioteca Rust equivalente apos avaliacao |
| SFTP | russh-sftp ou equivalente |
| GUI | egui/eframe como primeira avaliacao |
| Windowing/render | stack nativo do toolkit escolhido |
| Terminal engine | alacritty_terminal ou componente equivalente |
| Serializacao | Serde |
| Estado local | SQLite |
| Config exportavel | TOML ou JSON |
| Logs | tracing |
| Errors | thiserror / anyhow conforme camada |
| Credentials | credential store nativo via abstracao Rust |

### Restricao arquitetural

A escolha da GUI deve preservar o principio de nao embarcar um browser completo apenas para desenhar a interface.

---

## 17. Requisitos nao funcionais

### 17.1 Performance

Metas iniciais, sujeitas a benchmark real:

| Metrica | Meta de engenharia inicial |
|---|---:|
| RAM ao abrir | < 50 MB desejavel |
| RAM com 1 sessao | < 80 MB desejavel |
| RAM com 5 sessoes | < 150 MB desejavel |
| CPU idle | proximo de 0% |
| Startup em maquina moderna | ~1 s como objetivo |
| Sessao terminal | baixa latencia perceptiva |
| Sessoes simultaneas | 20+ como meta de stress |
| Transferencias grandes | streaming, sem carregar tudo em memoria |

Esses numeros sao objetivos de engenharia, nao garantias da primeira versao. Devem existir benchmarks automatizados ou reproduziveis para evitar regressao ao longo do desenvolvimento.

### 17.2 Confiabilidade

- Queda de uma sessao nao derruba as demais.
- Falha de SFTP nao encerra o terminal.
- Falha do monitor nao encerra a sessao SSH.
- Transferencias apresentam erro acionavel.
- Estado local deve resistir a encerramento inesperado.
- Aplicacao deve recuperar configuracoes validas apos crash.

### 17.3 Privacidade

- Nenhum login obrigatorio.
- Nenhum envio de lista de hosts para servidores externos.
- Nenhum envio de comandos executados para a Kernovae.
- Telemetria, caso exista no futuro, deve ser opt-in e claramente documentada.

---

## 18. Plataformas e distribuicao

### Matriz inicial de suporte

| Plataforma | Arquitetura | Prioridade |
|---|---|---|
| Windows 10/11 | x86_64 | P0 |
| Ubuntu LTS | x86_64 | P0 |
| Debian Stable | x86_64 | P0 |
| Fedora | x86_64 | P0 |
| Arch Linux | x86_64 | P1 |
| Linux | aarch64 | P1/P2 |
| Alpine/musl | a investigar | P2 |

### Artefatos de distribuicao

Primeira fase:

- Windows installer e/ou executavel assinado quando possivel.
- Binario Linux tarball.
- `.deb`.
- `.rpm`.

Fases posteriores podem avaliar AppImage e Flatpak, desde que sandboxing nao prejudique integracoes como SSH agent, credential store e acesso a arquivos locais.

A promessa do produto nao deve ser "funciona em qualquer Linux" sem testes. O projeto deve manter uma matriz publica de plataformas validadas em CI e testes de release.

---

## 19. MVP proposto - v0.1

### Dentro do MVP

- Gerenciamento de hosts SSH.
- Password auth.
- Key auth.
- Host key/fingerprint.
- Terminal funcional.
- Multiplas tabs.
- SFTP lateral.
- Navegacao de arquivos.
- Criar arquivo e diretorio.
- Renomear e excluir.
- Upload e download.
- Refresh.
- Gerenciador de transferencias.
- Persistencia local.
- Armazenamento seguro de credenciais.
- Windows e Linux.
- Tema claro/escuro.
- Monitoramento remoto basico:
  - CPU.
  - RAM.
  - swap.
  - load.
  - discos/filesystems.
  - mountpoints.
  - rede.
  - uptime.
  - informacoes do sistema.

### Fora do MVP inicial

- X11 forwarding.
- RDP.
- VNC.
- Kubernetes UI.
- Cloud sync.
- Conta Kernovae.
- Colaboracao de equipe.
- Historico de metricas de longo prazo.
- Alertas fora da sessao.
- Agente remoto permanente.
- Plugin marketplace.

---

## 20. Roadmap funcional inicial

### v0.1 - Core workstation

SSH + terminal + SFTP + transfers + monitoramento basico + persistencia + seguranca.

### v0.2 - Administracao avancada

- Jump hosts.
- Local/remote forwarding.
- Editor remoto aprimorado.
- Importacao de configuracao OpenSSH.
- Melhorias no inspector.
- Alertas locais por threshold.

### v0.3 - Power user

- Multi-exec.
- Macros/comandos salvos.
- Snippets.
- Process viewer.
- Layout split-pane.
- Session cloning.

### Futuro

- SOCKS proxy.
- X11 forwarding.
- Mosh, se fizer sentido para a arquitetura.
- Plugin API.
- RDP/VNC somente se nao comprometer o foco do produto.
- Sincronizacao opcional e auto-hospedavel, apenas se houver demanda real.

---

## 21. Requisitos de produto que nao devem ser quebrados

Estas restricoes definem a identidade do projeto:

1. O usuario deve conseguir instalar, abrir e usar o programa sem criar conta.
2. O programa deve funcionar sem backend da Kernovae.
3. O core nao deve depender de Electron.
4. O usuario deve poder administrar arquivos via SFTP visualmente.
5. O monitoramento remoto basico deve funcionar sem instalar agente permanente.
6. O terminal nao pode ser prejudicado pela coleta de metricas.
7. Windows e Linux sao plataformas de primeira classe.
8. Segredos nao podem ser armazenados em texto puro.
9. O produto deve continuar sendo util mesmo se a Kernovae desaparecer.
10. O projeto deve possuir uma estrategia de performance verificavel.

---

## 22. Naming do produto

A organizacao sera Kernovae, mas o gerenciador pode possuir um nome proprio.

Durante a pesquisa inicial foram descartados nomes com conflitos claros no mercado de terminal/SSH, incluindo **Termora**, **Shellora**, **Termyx** e **Conduit**. Existem produtos ativos com esses nomes ou extremamente proximos do mesmo dominio funcional.

### Direcao de naming recomendada

O nome deve ser:

- curto;
- facil de pronunciar em portugues e ingles;
- utilizavel como nome de binario;
- nao limitado apenas a SFTP;
- suficientemente diferente de Termius, MobaXterm, PuTTY e similares;
- coerente com a marca Kernovae;
- pesquisavel na web;
- adequado para um projeto open source.

### Candidatos iniciais

#### 1. Kervesh

**Leitura:** `ker-vesh`  
**Ideia:** combinacao conceitual de *kernel* + *mesh*. Remete a sistemas, hosts conectados e infraestrutura.  
**Marca:** `Kervesh by Kernovae`  
**Repo possivel:** `kernovae/kervesh`  
**Binario:** `kervesh`

Na verificacao inicial realizada em setembro de 2026, o handle exato `github.com/kervesh` retornou 404 e nao apareceu um produto de software obvio com o nome exato nas buscas realizadas. Isso e um bom sinal, mas nao equivale a uma busca formal de marca registrada.

#### 2. Nodvra

**Leitura:** `nod-vra`  
**Ideia:** nome inventado com referencia a *node* e infraestrutura.  
**Marca:** `Nodvra by Kernovae`  
**Repo possivel:** `kernovae/nodvra`  
**Binario:** `nodvra`

O handle `github.com/nodvra` tambem retornou 404 na verificacao inicial. O nome e mais abstrato, mas possui boa chance de ser diferenciavel.

#### 3. Kernovae Remote Workspace

Nome descritivo para documentacao e periodo inicial de desenvolvimento, sem comprometer a escolha da marca final. Pode ser usado ate o nome definitivo estar validado.

### Recomendacao atual

**Kervesh** e o candidato mais forte desta rodada porque possui relacao semantica com o produto sem tentar imitar nomes como "Termius" ou "MobaXterm". Tambem funciona bem como nome de executavel, crate prefix e repositorio.

Antes de publicar oficialmente, deve ser feita uma segunda verificacao de:

- GitHub;
- crates.io;
- dominios relevantes;
- mecanismos de busca;
- lojas de aplicativos relevantes;
- bases de marcas no Brasil e nos principais mercados pretendidos.

---

## 23. Posicionamento sugerido

Se o nome Kervesh for adotado:

> **Kervesh by Kernovae**  
> Native remote systems workspace.

Ou:

> **Kervesh**  
> SSH. SFTP. Systems. Native.

Ou, com foco filosofico:

> **Kervesh**  
> Your hosts. Your keys. Your machine.

Mensagem de produto:

> No mandatory account. No mandatory cloud. No web runtime. Native SSH, SFTP and remote monitoring for Windows and Linux.

---

## 24. Proximos passos recomendados

1. Escolher um codename temporario ou nome definitivo.
2. Criar o repositorio principal na organizacao Kernovae.
3. Criar `README.md`, `LICENSE`, `CONTRIBUTING.md`, `SECURITY.md` e `CODE_OF_CONDUCT.md`.
4. Definir a licenca open source.
5. Criar Cargo Workspace vazio com limites claros entre crates.
6. Realizar spike de GUI com terminal renderizado.
7. Realizar spike de SSH interativo.
8. Realizar spike de SFTP na mesma conexao.
9. Realizar spike de monitoramento Linux por canal SSH independente.
10. Medir memoria, startup e CPU desde o primeiro prototipo.
11. Fechar o backlog do v0.1 antes de adicionar protocolos extras.

---

## 25. Definicao de sucesso do primeiro prototipo

O primeiro prototipo tecnico pode ser considerado bem-sucedido quando, em Windows e Linux, for possivel:

1. abrir o aplicativo nativamente;
2. cadastrar um host;
3. conectar por SSH;
4. abrir um shell interativo funcional;
5. navegar pelos arquivos remotos via SFTP;
6. fazer upload e download;
7. visualizar CPU, RAM, load, discos/mountpoints e rede do host sem agente instalado;
8. abrir duas ou mais sessoes em tabs;
9. encerrar uma sessao sem afetar as demais;
10. permanecer com consumo de recursos compativel com a proposta lightweight.

Esse prototipo valida o risco tecnico central do produto antes da expansao do roadmap.
