# Decisão: Rust ou Go

**Decisão: Rust no agente, no cliente e no servidor. TypeScript só no painel web.**

O OmegaDesk é código próprio. Os projetos em [referencias.md](referencias.md) servem como estudo de arquitetura e não são copiados.

## Por que Rust

| Critério | Rust | Go |
|---|---|---|
| Latência de vídeo e entrada | Sem coletor de lixo, então não há pausas no meio do stream | O coletor de lixo causa pequenas pausas; dá para mitigar, mas exige cuidado |
| Captura de tela e codecs (DXGI, ScreenCaptureKit, PipeWire, NVENC, QSV, VAAPI) | Acesso direto às APIs do sistema e às bibliotecas C, sem custo extra | Precisa de cgo, que é lento em chamadas frequentes e complica a compilação cruzada |
| Integração com o desktop (serviço do Windows, UAC, tela de login, Wayland) | Crates maduros (`windows-rs`) e o RustDesk prova que funciona | Possível, mas o ecossistema é mais fraco |
| Segurança de memória num software que dá controle total da máquina | Garantida pelo compilador | Também é segura (tem coletor de lixo) |
| Interface gráfica nativa | Tauri, Slint, egui ou Flutter via FFI | Opções limitadas (Fyne, Wails) |
| Tamanho do binário do agente | Pequeno (alguns MB) | Maior, com o runtime embutido |
| Referências na mesma linguagem | RustDesk, IronRDP (RDP completo), LinuxPlay, SubnetDesk | NetBird, rdpgw e o agente do Tactical RMM, todos do lado servidor |
| Velocidade de desenvolvimento | Mais lenta, com curva de aprendizado | Mais rápida |
| Servidor de ID e relay | Bom (tokio, quinn para QUIC) | Excelente |

O Go seria a escolha certa se o OmegaDesk fosse só servidor e painel. Mas a parte difícil, e o que define se ele é "melhor que todos", fica no agente: capturar a tela, codificar o vídeo por hardware, transmitir com baixa latência e injetar teclado e mouse. Nisso o Rust vence com folga.

Escrever o servidor também em Rust permite **compartilhar o crate do protocolo** entre o agente, o cliente e o servidor. Isso elimina a duplicação de estruturas e uma linguagem a mais para manter.

## Divisão inicial

```
omegadesk/
├── crates/
│   ├── protocol/   # mensagens (protobuf), criptografia ponta a ponta, handshake
│   ├── capture/    # captura de tela por plataforma
│   ├── codec/      # H.264/H.265/AV1 por hardware, com VP9 em software como fallback
│   ├── input/      # injeção de teclado e mouse
│   ├── transport/  # QUIC + travessia de NAT (hole punching) + relay
│   ├── mesh/       # rede privada estilo Tailscale: WireGuard (boringtun), IPs e DNS internos, ACLs
│   └── rdp/        # compatibilidade com RDP (via IronRDP)
├── agent/          # serviço que roda na máquina acessada
├── client/         # aplicativo de quem acessa (Tauri)
├── server/         # ID, relay, API e gravação de sessões
└── web/            # painel e cliente pelo navegador (TypeScript + WebRTC/WebCodecs)
```
