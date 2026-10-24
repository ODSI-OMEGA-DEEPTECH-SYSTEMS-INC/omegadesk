# Projetos de referência

O OmegaDesk é escrito do zero, em Rust (veja [linguagem.md](linguagem.md)). Os projetos abaixo são estudados para aprender arquitetura, protocolos e soluções.

Ideias e arquitetura não têm licença. **Copiar código** tem: um trecho copiado só pode entrar se a licença for compatível com a AGPL-3.0, se for creditado e se manter o aviso de copyright original.

Licenças verificadas nos arquivos de cada repositório em 24/10/2026.

| Projeto | O que estudar | Licença | Copiar código? |
|---|---|---|---|
| [rustdesk/rustdesk](https://github.com/rustdesk/rustdesk) | Arquitetura do cliente, travessia de NAT, codecs, serviço no Windows | AGPL-3.0 | ✅ |
| [rustdesk/rustdesk-server](https://github.com/rustdesk/rustdesk-server) | Servidor de ID (hbbs) e relay (hbbr) | AGPL-3.0 | ✅ |
| [zibo-chen/SubnetDesk](https://github.com/zibo-chen/SubnetDesk) | Modo só rede local, sem servidor | AGPL-3.0 | ✅ |
| [Ylianst/MeshCentral](https://github.com/Ylianst/MeshCentral) | Painel web, gestão de agentes, grupos e permissões | Apache-2.0 | ✅ |
| [amidaware/tacticalrmm](https://github.com/amidaware/tacticalrmm) | Funcionalidades de RMM (scripts, alertas, inventário) | Tactical RMM License (proprietária) | ❌ só ideias |
| [kunkundi/crossdesk](https://github.com/kunkundi/crossdesk) | Cliente web leve | GPL-3.0 | ✅ |
| [galaxy-s10/billd-desk](https://github.com/galaxy-s10/billd-desk) | Pipeline com WebRTC | MIT | ✅ |
| [miroslavpejic85/p2p](https://github.com/miroslavpejic85/p2p) | Modo portátil, sem instalação | MIT | ✅ |
| [dchapyshev/aspia](https://github.com/dchapyshev/aspia) | Relay, transferência de arquivos, catálogo de endereços | GPL-3.0 | ✅ |
| [barry-ran/QuickDesk](https://github.com/barry-ran/QuickDesk) | Integração com IA via MCP | MIT | ✅ |
| [immense/Remotely](https://github.com/immense/Remotely) | Suporte remoto sem instalação (parado desde 12/2024) | GPL-3.0 | ✅ |
| [apache/guacamole-server](https://github.com/apache/guacamole-server) + [client](https://github.com/apache/guacamole-client) | Gateway pelo navegador, gravação de sessões | Apache-2.0 | ✅ |
| [jumpserver/jumpserver](https://github.com/jumpserver/jumpserver) | Controle de acesso privilegiado (PAM), auditoria, aprovação de acesso | GPL-3.0 | ✅ |
| [kasmtech/KasmVNC](https://github.com/kasmtech/KasmVNC) | VNC pelo navegador com bom desempenho | GPL-2.0-ou-posterior | ✅ |
| [novnc/noVNC](https://github.com/novnc/noVNC) | Cliente VNC em JavaScript | MPL-2.0 | ✅ |
| [selkies-project/selkies](https://github.com/selkies-project/selkies) | Streaming WebRTC com GPU, containers | MPL-2.0 | ✅ |
| [FreeRDP/FreeRDP](https://github.com/FreeRDP/FreeRDP) | Protocolo RDP completo | Apache-2.0 | ✅ |
| [Devolutions/IronRDP](https://github.com/Devolutions/IronRDP) | RDP em Rust; pode ser usado como dependência | MIT/Apache-2.0 | ✅ |
| [neutrinolabs/xrdp](https://github.com/neutrinolabs/xrdp) | Servidor RDP para Linux | Apache-2.0 | ✅ |
| [TigerVNC/tigervnc](https://github.com/TigerVNC/tigervnc) | Protocolo VNC/RFB | GPL-2.0-ou-posterior | ✅ |
| [ultravnc/UltraVNC](https://github.com/ultravnc/UltraVNC) | Recursos do VNC no Windows | GPL-3.0 | ✅ |
| [mRemoteNG/mRemoteNG](https://github.com/mRemoteNG/mRemoteNG) | Gerenciador de conexões em abas | GPL-2.0-ou-posterior | ✅ |
| [FreeRDP/Remmina](https://github.com/FreeRDP/Remmina) | Cliente multiprotocolo para Linux | GPL-2.0-ou-posterior | ✅ |
| [1Remote/1Remote](https://github.com/1Remote/1Remote) | Inicializador rápido de conexões | GPL-3.0 | ✅ |
| [LizardByte/Sunshine](https://github.com/LizardByte/Sunshine) | Codificação por hardware com baixa latência | GPL-3.0 | ✅ |
| [moonlight-stream/moonlight-qt](https://github.com/moonlight-stream/moonlight-qt) | Decodificação e exibição com baixa latência | GPL-3.0 | ✅ |
| [ClassicOldSong/Apollo](https://github.com/ClassicOldSong/Apollo) | Tela virtual na resolução do cliente | GPL-3.0 | ✅ |

## Cópias locais

Clones rasos (`--depth 1`) em `C:\Users\Ricardo.Bueno\omegadesk-ref\`. Ficam fora deste repositório de propósito.

## Rede mesh (estilo Tailscale)

Objetivo: cada agente e cliente do OmegaDesk entra numa rede privada criptografada, alcançando qualquer máquina pelo IP ou nome interno, atravessando NAT sem abrir portas, com relay próprio quando a conexão direta falhar.

| Projeto | O que estudar | Linguagem | Licença | Copiar código? |
|---|---|---|---|---|
| [tailscale/tailscale](https://github.com/tailscale/tailscale) | Referência do mercado: DERP (relay), disco (descoberta de caminho), MagicDNS, ACLs, SSH | Go | BSD-3 | ✅ |
| [juanfont/headscale](https://github.com/juanfont/headscale) | Servidor de controle do Tailscale, self-hosted | Go | BSD-3 | ✅ |
| [netbirdio/netbird](https://github.com/netbirdio/netbird) | Gerência, signal e relay separados; ICE; políticas e checagem de postura | Go | BSD-3 no cliente, AGPL-3.0 no servidor | ✅ |
| [EasyTier/EasyTier](https://github.com/EasyTier/EasyTier) | **Mesh descentralizada em Rust**, sem servidor central obrigatório, WireGuard, QUIC, KCP | Rust | LGPL-3.0 | ✅ |
| [cloudflare/boringtun](https://github.com/cloudflare/boringtun) | **WireGuard em Rust (userspace)**; pode ser dependência direta | Rust | BSD-3 | ✅ |
| [tonarino/innernet](https://github.com/tonarino/innernet) | Rede privada sobre WireGuard com CIDRs hierárquicos | Rust | MIT | ✅ |
| [DefGuard/defguard](https://github.com/DefGuard/defguard) | WireGuard com MFA e acesso zero-trust | Rust | AGPL-3.0 (núcleo), `enterprise/` proprietário | ✅ exceto `enterprise/` |
| [slackhq/nebula](https://github.com/slackhq/nebula) | Overlay com certificados e lighthouses | Go | MIT | ✅ |
| [zerotier/ZeroTierOne](https://github.com/zerotier/ZeroTierOne) | Rede virtual Ethernet (camada 2) | C++ | MPL-2.0, `nonfree/` proprietário | ✅ exceto `nonfree/` |
| [firezone/firezone](https://github.com/firezone/firezone) | Acesso remoto zero-trust; o núcleo de conexão (connlib) é em Rust | Elixir + Rust | Apache-2.0 | ✅ |
| [gravitl/netmaker](https://github.com/gravitl/netmaker) | Automação de redes WireGuard | Go | Apache-2.0, `pro/` proprietário | ✅ exceto `pro/` |

Observação: o headscale foi extraído sem `hscontrol/testdata/apiv1_golden/`, que tem nomes de arquivo inválidos no Windows.
