# OmegaDesk

Acesso remoto open source e self-hosted da **ODSI – Omega DeepTech Systems**, uma alternativa ao TeamViewer e ao AnyDesk.

O OmegaDesk é escrito do zero, em **Rust** ([por quê](docs/linguagem.md)), reunindo num só produto o que há de melhor nos projetos abertos de acesso remoto:

| Área | Inspiração principal |
|---|---|
| Núcleo do cliente, ID/relay próprio, P2P com NAT traversal | [RustDesk](https://github.com/rustdesk/rustdesk) + [rustdesk-server](https://github.com/rustdesk/rustdesk-server) |
| Painel web de gestão de máquinas e agentes | [MeshCentral](https://github.com/Ylianst/MeshCentral) |
| Acesso pelo navegador, sem instalar cliente | [Apache Guacamole](https://github.com/apache/guacamole-server), [noVNC](https://github.com/novnc/noVNC), [Selkies](https://github.com/selkies-project/selkies) |
| Auditoria e gravação de sessões | [JumpServer](https://github.com/jumpserver/jumpserver) |
| Streaming de baixa latência e codificação por hardware | [Sunshine](https://github.com/LizardByte/Sunshine), [Moonlight](https://github.com/moonlight-stream/moonlight-qt), [Apollo](https://github.com/ClassicOldSong/Apollo) |
| Compatibilidade com RDP e VNC | [IronRDP](https://github.com/Devolutions/IronRDP), [FreeRDP](https://github.com/FreeRDP/FreeRDP), [TigerVNC](https://github.com/TigerVNC/tigervnc) |
| Rede privada mesh (estilo Tailscale), sem abrir portas | [Tailscale](https://github.com/tailscale/tailscale), [Headscale](https://github.com/juanfont/headscale), [NetBird](https://github.com/netbirdio/netbird), [EasyTier](https://github.com/EasyTier/EasyTier), [boringtun](https://github.com/cloudflare/boringtun) |
| Gerenciador de conexões em abas | [mRemoteNG](https://github.com/mRemoteNG/mRemoteNG), [1Remote](https://github.com/1Remote/1Remote) |

A lista completa, com licenças e o que pode ou não ser reaproveitado, está em [docs/referencias.md](docs/referencias.md). O plano de construção, com a arquitetura e as fases, está em [docs/plano.md](docs/plano.md).

## Licença

[AGPL-3.0](LICENSE), a mesma do RustDesk. Ela permite incorporar código sob AGPL-3.0, GPL-3.0, GPL-2.0-ou-posterior, Apache-2.0, MPL-2.0 e MIT. Código sob **GPL-2.0-only** ou licenças proprietárias **não** pode ser copiado para cá.

## Como testar (Fase 1)

Na máquina que vai ser acessada (Windows):

```sh
cargo run --release -p omegadesk-agent -- --listen 0.0.0.0:7070
```

O agente mostra uma **impressão digital**. Na máquina de onde você vai acessar:

```sh
cargo run --release -p omegadesk-client -- 192.168.0.10:7070 --fingerprint <impressão digital>
```

A barra de título do cliente mostra a latência: tempo no host, rede, decodificação e desenho. Use `--view-only` no cliente para só assistir, ou `--no-input` no agente para não aceitar teclado e mouse.

Primeira medição (Intel Core Ultra 5 135U, 1920x1080, agente e cliente na mesma máquina, H.264 por software): **24 fps e cerca de 48 ms de latência total**, dos quais ~40 ms são de codificação. O encoder por hardware é o próximo passo para chegar à meta de 30 ms.
