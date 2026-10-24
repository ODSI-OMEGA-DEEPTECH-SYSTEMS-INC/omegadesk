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
