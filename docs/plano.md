# Plano de construção do OmegaDesk

Este plano junta a análise do código dos 39 repositórios de referência (lista em [referencias.md](referencias.md)). Cada decisão indica de onde veio a ideia. Ideias e arquitetura são livres; **código só é copiado quando a licença permite**, e com atribuição.

---

## 1. Princípios

1. **Latência primeiro.** Captura, codificação e envio sem passar pela CPU sempre que houver GPU. Cada quadro carrega o próprio tempo de processamento, e o cliente mostra o atraso total.
2. **Rede própria.** Toda máquina entra numa malha privada criptografada. Ninguém abre porta no roteador; o relay é nosso.
3. **Zero confiança.** Identidade forte por dispositivo, política central, consentimento do usuário remoto, gravação e auditoria.
4. **Um produto só.** Acesso remoto, suporte rápido, RMM, PAM e VPN no mesmo agente, no mesmo servidor e no mesmo painel.
5. **Código limpo.** Crates pequenos, núcleo de rede *sans-I/O* (testável sem rede), nada de arquivo com 5 mil linhas, nada de configuração global estática. São os defeitos que vimos no RustDesk, no MeshCentral e no EasyTier.

---

## 2. O melhor de cada repositório

### 2.1 Conexão e protocolo

| O que adotar | De onde | Como fica no OmegaDesk |
|---|---|---|
| Envelope protobuf com `oneof` e codec negociado (`SupportedEncoding`) | RustDesk `libs/base/protos/message.proto` | Mesmo desenho, em `crates/protocol`, com versão de protocolo explícita |
| Corrida de transportes: tenta TCP, UDP furado, IPv6 e WebRTC ao mesmo tempo e fica com o primeiro | RustDesk `src/client.rs` (`race_transports_prefer_webrtc`) | Corrida entre caminho direto da malha, relay e WebSocket |
| Relay sem estado com tickets de uso único emitidos pelo coordenador | Aspia (`RelayKeyPool`, `ConnectionOffer`) | Relays sem banco de dados, escaláveis horizontalmente |
| Modo só rede local: mDNS, handshake com transcrição assinada, confiança no primeiro uso (TOFU) | SubnetDesk (`lan_mdns.rs`, `lan_protocol.rs`, `lan_identity.rs`) | Funciona sem internet e sem servidor |
| Suporte rápido de executável único com consentimento | UltraVNC SingleClick, Remotely | "OmegaDesk QuickSupport": baixa, roda, mostra o ID, o usuário aceita |

**Não copiar:** a criptografia do RustDesk (`sodiumoxide` está descontinuado e o nonce é fixo em zero) e a "chave de licença" global do rustdesk-server.

### 2.2 Rede mesh (estilo Tailscale)

| O que adotar | De onde | Como fica no OmegaDesk |
|---|---|---|
| Criptografia WireGuard em Rust, só o núcleo `noise::Tunn` | boringtun 0.7 (BSD-3) | **Dependência direta**, com I/O próprio em tokio |
| Biblioteca de conexão *sans-I/O*: ICE + WireGuard | Firezone `snownet` (Apache-2.0) | Modelo do `crates/mesh`; crates `str0m`/`is` para ICE |
| Placa de rede virtual por plataforma (wintun com offload) | Firezone `tun-windows`, `tun-apple`, `tun-linux` | Um crate por plataforma |
| Mensagens de descoberta (Ping, Pong, CallMeMaybe) e troca contínua do relay pelo caminho direto | Tailscale `disco/`, `wgengine/magicsock/` | "magicsock" próprio, num socket UDP só |
| Relay endereçado por chave pública, sobre HTTPS na porta 443, escolhendo a região mais próxima | Tailscale DERP | `omegadesk-relay` |
| Mapa da rede enviado por stream com deltas e agrupamento de mudanças | Headscale `hscontrol/mapper/batcher.go` | Stream de netmap no coordenador |
| Políticas em HuJSON (grupos, tags, grants) compiladas em filtro por nó | Tailscale/Headscale `policy/v2` | Mesmo formato, para quem já conhece o Tailscale |
| Checagem de postura (SO, versão, geolocalização, processo) e conexões sob demanda | NetBird `posture/`, `lazyconn/` | Política só libera se a máquina estiver em conformidade |
| Grupos e identidade assinados no certificado; relay feito por qualquer nó | Nebula | Fase 2 |
| Roteamento entre peers sem servidor central (OSPF) | EasyTier | Fase 2, só como inspiração (licença LGPL) |
| DNS interno (MagicDNS) | Tailscale `net/dns`, EasyTier, Firezone | `nome-da-maquina.omega` |

**Decisão:** protocolo de controle **próprio**, com semântica inspirada no `tailcfg`. Compatibilidade com o app do Tailscale só depois, como gateway separado, porque o protocolo deles não é documentado e muda a cada versão.

**Não copiar:** controller do ZeroTier (`nonfree/`), código de servidor do NetBird (AGPL), `pro/` do Netmaker, cifra `xor` do EasyTier e "NAT traversal" sequencial do innernet.

### 2.3 Vídeo e baixa latência

| O que adotar | De onde | Como fica no OmegaDesk |
|---|---|---|
| Captura no Windows: Desktop Duplication (`DuplicateOutput1`, FP16 para HDR), com WGC e GDI como fallback | Sunshine `display_base.cpp`, CrossDesk | `crates/capture` com ordem de fallback |
| `SetMaximumFrameLatency(1)` e prioridade de GPU alta | Sunshine | Padrão ligado |
| Conversão de cor em shader, entregando a textura direto ao encoder | Sunshine `display_vram.cpp`, RustDesk `hwcodec` (modo vram) | Caminho zero-copy |
| NVENC `ULTRA_LOW_LATENCY`, sem B-frames, sem lookahead; AMF ULTRA_LOW_LATENCY; QSV `async_depth=1`; VAAPI ou Vulkan Video no Linux | Sunshine `nvenc_base.cpp`, `video.cpp`, `vulkan_encode.cpp` | Encoder por hardware atrás de uma trait |
| Em caso de perda, invalidar quadros de referência ou fazer intra-refresh em vez de mandar quadro completo (IDR) | Sunshine `invalidate_ref_frames` | Recuperação sem picos de banda |
| Correção de erros Reed-Solomon adaptativa sobre UDP | Sunshine, Apollo (AVX2/AVX512) | Crate `reed-solomon-simd` |
| Tempo de processamento no cabeçalho de cada quadro e painel de latência no cliente | Sunshine `frame_processing_latency`, Moonlight `ffmpeg.cpp` | Overlay: host, rede, decodificação e renderização |
| Controle de qualidade: mede atraso, reduz primeiro o bitrate e depois o fps | RustDesk `video_qos.rs` | Combinado com confirmação de recebimento de quadro (do Selkies) |
| Detecta "parece vídeo" pela área alterada; conteúdo parado em qualidade sem perdas | KasmVNC `EncodeManager.cxx`, Selkies "paint-over" | Texto nítido e vídeo fluido |
| Tela virtual com a resolução e o FPS do cliente, identidade estável e modo sem monitor | Apollo `virtual_display.cpp` | Driver IddCx próprio (o SudoVDA é GPL) |
| Apresentação de baixa latência: flip model, tearing permitido, fila de 1 a 3 quadros | Moonlight `d3d11va.cpp`, `pacer.cpp` | Cliente nativo |
| Áudio Opus em `RESTRICTED_LOWDELAY` | Sunshine `audio.cpp` | Padrão |

**Não copiar:** o protocolo GameStream, o pareamento por PIN e a ideia de sessão como "app" (Sunshine/Apollo); a captura por `CopyFromScreen` e o codec JPEG do p2p; o driver espelho do UltraVNC.

### 2.4 Windows: serviço, tela de login e UAC

| O que adotar | De onde |
|---|---|
| Serviço SYSTEM que lança o processo de captura na sessão ativa com o token do `winlogon.exe` (`CreateProcessAsUserW`), o que permite capturar a tela de login e o UAC | RustDesk `src/platform/windows.rs`, `windows.cc` |
| Tratamento explícito da troca para o Secure Desktop e da passagem de sessão RDP | CrossDesk `secure_desktop_dxgi_capture.h`, `rdp_session_handoff.cpp` |
| Separação entre o serviço do agente e o processo por sessão de usuário | Remotely, CrossDesk |
| Modo privacidade (tela preta para quem está na frente da máquina) | RustDesk `src/privacy_mode/` |

### 2.5 Navegador (cliente web)

| O que adotar | De onde |
|---|---|
| H.264/H.265/AV1 decodificado com WebCodecs dentro de Worker + OffscreenCanvas, sem MSE e sem jitter buffer | Selkies `selkies-ws-core.js`, noVNC `core/decoders/h264.js`, SubnetDesk `video-worker.ts` |
| Escada de codecs conforme `VideoDecoder.isConfigSupported` | Selkies |
| Confirmação de recebimento por quadro (ACK) para medir o tempo de ida e volta e descartar quadros de cliente lento | Selkies `websockets_mode.py` |
| WebTransport ou WebRTC DataChannel, com fallback para WebSocket; mensagens binárias tipadas pelo primeiro byte | Selkies, KasmVNC `webudp` |
| Teclado: enviar scancode + keysym, que resolve layouts internacionais (ABNT2 incluso) | noVNC (QEMU ext key), Guacamole `Keyboard.js` |
| Gateway WebSocket em Rust (Axum + rustls) com checagem de Origin e limite por IP | SubnetDesk `web_gateway.rs` |
| RDP no navegador via WASM | IronRDP `ironrdp-web` + RDCleanPath |

**Não copiar:** o modelo do Guacamole de decodificar e recodificar em imagens (dobra a latência), o Java/Tomcat no meio e o áudio PCM sem compressão.

### 2.6 Compatibilidade RDP e VNC

| O que adotar | De onde |
|---|---|
| **Cliente RDP** para máquinas Windows existentes, alcançadas pela malha | IronRDP como dependência: `ironrdp-connector`, `-session`, `-tokio`, `-graphics`, `-egfx`, `-cliprdr`, `-rdpdr`, `-rdpsnd`, `-displaycontrol` |
| **Servidor RDP** alimentado pela nossa própria captura (fase posterior) | `ironrdp-server` (feature `egfx`) + `ironrdp-acceptor` |
| Testes de interoperabilidade | FreeRDP (referência de conformidade, não dependência) |
| Sessão virtual por usuário no Linux | xrdp (modelo) |
| Algoritmo de congestionamento e escolha do encoder por região | TigerVNC `Congestion.cxx`, `EncodeManager.cxx` |
| Modo de compatibilidade VNC/RFB | TigerVNC (fase posterior) |

As versões do IronRDP ficam fixadas exatamente (a API ainda é 0.x).

### 2.7 Gestão, permissões e multi-tenant

| O que adotar | De onde |
|---|---|
| Hierarquia tenant > grupo de dispositivos > dispositivo, mais grupos de usuários | MeshCentral (domínios), Remotely, Tactical RMM (Cliente > Site > Agente) |
| Permissões em bitflags (ver só, sem terminal, sem arquivos, input limitado, compartilhar com convidado…) num crate único | MeshCentral `MESHRIGHT_*` e `SITERIGHT_*` |
| "Admin" como papel explícito, nunca "todos os bits ligados" | Corrige um defeito do MeshCentral (`0xFFFFFFFF`) |
| Consentimento do usuário remoto em camadas: tenant > grupo > máquina > usuário | MeshCentral |
| Permissões por cliente: clipboard, upload, comandos, só visualizar | Apollo `PERM` |
| Papéis com flags `can_*` e escopo por cliente/site | Tactical RMM (só a ideia) |
| Vários domínios e certificados (SNI) por tenant | MeshCentral `webserver.js` |

### 2.8 RMM (gestão de máquinas)

| O que adotar | De onde |
|---|---|
| Políticas herdáveis aplicadas a cliente, site ou máquina | Tactical RMM (só a ideia) |
| Checks → alertas → templates de alerta com ação de falha e de resolução | Tactical RMM (só a ideia) |
| Links de implantação por site com token que expira | Tactical RMM (só a ideia), MeshCentral |
| Inventário, campos customizados, Windows Update, scripts agendados | Tactical RMM, Remotely `ScriptScheduler.cs` |
| Atualização automática do agente: o agente manda o hash do binário e o servidor envia o novo em blocos, com limite de concorrência | MeshCentral `meshagent.js` |
| Barramento de mensagens com tópico amarrado à identidade do agente | Tactical RMM (NATS) → no OmegaDesk, sobre QUIC na malha |

### 2.9 Auditoria, gravação e acesso privilegiado (PAM)

| O que adotar | De onde |
|---|---|
| Gravação feita no relay, sem depender do agente, em formato append-only com cabeçalho TLV + timestamp e índice de keyframes para avançar e voltar | MeshCentral `.mcrec`, `meshrelay.js`; Guacamole `recording.c` |
| Terminal gravado em asciicast `.cast`, para reaproveitar players existentes | JumpServer |
| Comandos digitados indexados à parte, com busca | JumpServer `CommandStorage` |
| Player de gravação no navegador | Guacamole `SessionRecording.js` |
| Pedido de acesso com aprovação em vários níveis (Ticket com etapas) | JumpServer `apps/tickets/` |
| Regras de login, de comando e de clipboard com ações graduadas: aceitar, rejeitar, revisar, avisar | JumpServer `apps/acls/` |
| Acompanhar sessão ao vivo com código, prazo e modo só leitura; travar ou encerrar sessão | JumpServer `sharing.py`, MeshCentral `meshdesktopmultiplex.js` |
| Prevenção de vazamento (limite de clipboard, taxa de teclas, região) e marca d'água | KasmVNC DLP |

### 2.10 Autenticação

| O que adotar | De onde |
|---|---|
| OIDC, SAML, LDAP, TOTP, passkeys/WebAuthn, bloqueio de força bruta | Guacamole `extensions/`, JumpServer `authentication/`, defguard (só a ideia) |
| MFA por conexão, não só por login | defguard (só a ideia) |
| Chaves pré-autorizadas e efêmeras para implantar máquinas | Headscale `preauth_keys.go` |
| Senha nunca trafega: SRP ou OPAQUE para a senha de acesso não supervisionado | Aspia (SRP) |
| Chave de máquina persistente + chave de nó com rotação e validade | Tailscale |

### 2.11 Gerenciador de conexões e cofre

| O que adotar | De onde |
|---|---|
| Árvore de pastas com herança de propriedades **e** tags com filtro | mRemoteNG, 1Remote |
| Inicializador por busca de teclado | 1Remote `KeywordMatchService.cs` |
| Importar RDCMan, `.rdp`, PuTTY e Active Directory | mRemoteNG `MiscSerializers/`, IronRDP `ironrdp-rdpfile` |
| Cofre de credenciais de verdade (envelope encryption, Argon2id) e keyring do sistema no cliente desktop | Remmina (keyring); corrige o `UnSafeStringEncipher` do 1Remote |

### 2.12 Diferenciais novos

| O que adotar | De onde |
|---|---|
| Servidor **MCP** para agentes de IA (screenshot, clique, digitação) com controle de confiança por máquina | QuickDesk `quickdesk-mcp/` (crate `rmcp`) |
| Grade com vários desktops ao vivo | CrossDesk DeskHub |
| Painel com webhooks e auditoria | QuickDesk `SignalingServer/internal/models/` |

---

## 3. Arquitetura

```
                        ┌──────────────────────────────────────────┐
                        │            omegadesk-server              │
                        │  coordenador: contas, tenants, política, │
                        │  netmap, tickets de relay, RMM, PAM, API │
                        │  (Axum + PostgreSQL/SQLite)              │
                        └──────┬──────────────────────┬────────────┘
              stream de netmap │ (Noise IK sobre QUIC) │ painel / API
                               │                      │
   ┌───────────────────┐  ┌────┴───────────────┐   ┌──┴──────────────────┐
   │  omegadesk-relay  │  │  omegadesk-agent   │   │   web (TypeScript)  │
   │  sem estado,      │  │  serviço SYSTEM    │   │   painel + cliente  │
   │  por chave pública│◄─┤  + worker de sessão│   │   WebCodecs/Worker  │
   │  HTTPS 443, grava │  │  captura, encoder, │   └─────────────────────┘
   │  sessões          │  │  input, RMM, mesh  │
   └───────────────────┘  └────────────────────┘
            ▲                      ▲  WireGuard (boringtun) + disco + relay
            └──────────┬───────────┘
              ┌────────┴─────────┐
              │ omegadesk-client │  app desktop (Tauri) com decodificação
              │ + QuickSupport   │  por hardware, gerenciador de conexões
              └──────────────────┘
```

### Workspace Rust

```
omegadesk/
├── crates/
│   ├── protocol/      # protobuf (prost), versões, envelope oneof
│   ├── identity/      # chaves ed25519/x25519, chave de máquina e de nó, TOFU
│   ├── noise/         # canal de controle Noise IK (snow)
│   ├── mesh/          # sans-I/O: WireGuard (boringtun), disco, ICE (str0m), relay
│   ├── tun/           # wintun / utun / tun-linux
│   ├── transport/     # QUIC (quinn), corrida de rotas, FEC Reed-Solomon
│   ├── capture/       # DXGI → WGC → GDI; PipeWire/KMS; ScreenCaptureKit
│   ├── codec/         # trait Encoder/Decoder: NVENC, AMF, QSV, VAAPI, VideoToolbox, software
│   ├── input/         # injeção de teclado, mouse, toque, caneta
│   ├── session/       # vídeo, áudio, clipboard, arquivos, chat, terminal
│   ├── recording/     # formato .odrec (TLV + índice), asciicast
│   ├── policy/        # HuJSON → filtro por nó; permissões em bitflags
│   ├── rdp/           # integração IronRDP
│   └── mcp/           # servidor MCP (rmcp)
├── agent/             # binário do agente (serviço + worker de sessão)
├── client/            # app Tauri
├── quicksupport/      # executável único de suporte rápido
├── server/            # coordenador
├── relay/             # relay sem estado
├── drivers/
│   └── idd/           # tela virtual IddCx (Windows)
└── web/               # painel + cliente web (TypeScript)
```

### Dependências principais

| Função | Crate | Observação |
|---|---|---|
| Async | `tokio` | |
| WireGuard | `boringtun` 0.7, só `noise::Tunn` | sem o módulo `device` |
| ICE | `str0m` / `is` | desenho do Firezone `snownet` |
| QUIC | `quinn` + `quinn-udp` | GSO/GRO |
| Noise | `snow` | canal de controle |
| TUN Windows | `wintun` | |
| Protobuf | `prost` | |
| HTTP/Web | `axum`, `rustls`, `tokio-tungstenite` | |
| Banco | `sqlx` (PostgreSQL e SQLite) | |
| FEC | `reed-solomon-simd` | |
| Áudio | `opus` | |
| RDP | `ironrdp-*` | versões fixadas |
| MCP | `rmcp` | |
| UI desktop | `tauri` | |
| Permissões | `bitflags` | |

---

## 4. Fases de construção

Cada fase termina com algo que roda e pode ser demonstrado.

### Fase 0: Fundação (1–2 semanas)
- Workspace Rust, CI (build Windows, Linux e macOS; `clippy` com lints rígidos no estilo do Firezone; `cargo deny` para licenças).
- `protocol` com as primeiras mensagens; `identity` com chaves e armazenamento seguro.
- **Pronto quando:** CI verde nas três plataformas e `cargo deny` bloqueando licença incompatível.

### Fase 1: Primeira imagem na tela (3–4 semanas)
- Agente captura a tela no Windows (DXGI), codifica com NVENC/QSV/AMF ou em software e envia por QUIC direto.
- Cliente nativo decodifica e mostra; mouse e teclado funcionando.
- Overlay de latência (host, rede, decodificação e renderização).
- **Pronto quando:** controle de outra máquina na mesma rede local, com latência **≤ 30 ms** na LAN em 1080p60.

### Fase 2: Rede mesh e servidor (4–6 semanas)
- Coordenador: cadastro de máquinas com chave pré-autorizada, stream de netmap, OIDC.
- `mesh`: WireGuard (boringtun), disco, STUN/UPnP, troca do relay pelo caminho direto.
- Relay sem estado na porta 443 com tickets de uso único.
- DNS interno `*.omega`.
- **Pronto quando:** duas máquinas atrás de NATs diferentes se conectam sem abrir porta; quando a conexão direta falha, cai no relay em menos de 2 s.

### Fase 3: Produto de acesso remoto (4–6 semanas)
- Serviço Windows SYSTEM com captura da tela de login e do UAC.
- Clipboard (texto e arquivos), transferência de arquivos, áudio, chat, vários monitores.
- Controle de qualidade adaptativo, FEC e invalidação de quadros de referência.
- QuickSupport (executável único com ID e consentimento).
- Modo LAN sem servidor (mDNS + TOFU).
- **Pronto quando:** substitui o RustDesk/AnyDesk no suporte do dia a dia da Omega.

### Fase 4: Painel web e cliente no navegador (4–6 semanas)
- Painel: tenants, grupos, máquinas, usuários, permissões em bitflags, consentimento em camadas.
- Cliente web com WebCodecs em Worker, ACK por quadro, WebTransport e fallback para WebSocket.
- Gerenciador de conexões (árvore + tags + busca) e importadores.
- **Pronto quando:** acesso completo pelo navegador com latência no máximo 15 ms acima da do cliente nativo.

### Fase 5: Segurança, auditoria e PAM (4 semanas)
- Gravação de sessão no relay (`.odrec` + asciicast) e player no navegador.
- Políticas HuJSON, checagem de postura, MFA por conexão.
- Pedido de acesso com aprovação, acompanhar sessão ao vivo, travar e encerrar sessão.
- DLP e marca d'água; cofre de credenciais.
- **Pronto quando:** toda sessão fica gravada e pesquisável, e um acesso a servidor crítico exige aprovação.

### Fase 6: RMM (4–6 semanas)
- Inventário, scripts (PowerShell/bash) com agendamento, checks → alertas → templates.
- Políticas herdáveis, links de implantação por site, atualização automática do agente.
- **Pronto quando:** consegue gerenciar o parque inteiro de um cliente sem outra ferramenta.

### Fase 7: Compatibilidade e diferenciais (contínuo)
- Cliente RDP pela malha (IronRDP) e RDP no navegador.
- Tela virtual IddCx com a resolução do cliente; modo sem monitor.
- Servidor MCP para agentes de IA.
- Captura Linux (PipeWire/KMS) e macOS (ScreenCaptureKit); apps Android e iOS.
- Servidor RDP e VNC; relay entre peers e roteamento sem coordenador.

---

## 5. Metas para ser "melhor que todos"

| Métrica | Meta | Referência atual |
|---|---|---|
| Latência na LAN, 1080p60, ponta a ponta | ≤ 30 ms | Moonlight/Sunshine ~20–40 ms |
| Latência pelo navegador | ≤ nativo + 15 ms | Guacamole/noVNC bem acima disso |
| Tempo para conectar (P2P) | ≤ 1 s | |
| Queda do direto para o relay | ≤ 2 s | |
| Tamanho do agente | ≤ 15 MB | |
| CPU do agente em tela parada | ≈ 0 % | |
| Instalação do servidor | 1 comando, 1 binário + banco | |

---

## 6. Riscos e cuidados

- **Patentes de H.264/H.265.** Preferir encoders de hardware (a licença vem com a GPU) e oferecer AV1 e VP9 como alternativas livres.
- **Drivers no Windows.** A tela virtual IddCx e os drivers em geral precisam de assinatura da Microsoft (EV + attestation). Planejar com antecedência.
- **Antivírus.** Software de acesso remoto é visado. Assinar todos os binários desde o início e evitar técnicas que parecem malware, como carregar DLL da memória (o UltraVNC faz isso).
- **Dependências 0.x.** IronRDP, `str0m` e `rmcp` mudam de API. Fixar versões e isolar cada uma atrás de um crate próprio.
- **Escopo.** Sete fases são muito trabalho. As fases 1 a 3 já entregam um produto que substitui o RustDesk; as demais podem vir por ordem de prioridade comercial.
- **Licenças.** O `cargo deny` no CI bloqueia dependência incompatível. Nunca abrir as pastas proprietárias (`enterprise/`, `nonfree/`, `pro/`, Tactical RMM) como fonte.

---

## 7. Lacunas desta análise

- Os submódulos `libs/hbb_common` (RustDesk), `moonlight-common-c` (Moonlight), `kasmweb` (KasmVNC) e `proto/` (defguard) vieram vazios nos clones. O protocolo de rendezvous do RustDesk foi deduzido pelo código do servidor.
- A licença do fork `rustdesk-org/hwcodec` ainda não foi conferida.
- Nada foi compilado nem medido; as metas da seção 5 precisam ser validadas na Fase 1.
