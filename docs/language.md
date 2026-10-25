# Decision: Rust or Go


**Decision: Rust for the agent, the client and the server. TypeScript only for the web dashboard.**

OmegaDesk is original code. The projects in [references.md](references.md) serve as architecture study material and are not copied.

## Why Rust

| Criterion | Rust | Go |
|---|---|---|
| Video and input latency | No garbage collector, so no pauses mid-stream | The garbage collector causes short pauses; they can be mitigated, but it takes care |
| Screen capture and codecs (DXGI, ScreenCaptureKit, PipeWire, NVENC, QSV, VAAPI) | Direct access to system APIs and C libraries, with no extra overhead | Requires cgo, which is slow for frequent calls and complicates cross-compilation |
| Desktop integration (Windows service, UAC, login screen, Wayland) | Mature crates (`windows-rs`), and RustDesk proves it works | Possible, but the ecosystem is weaker |
| Memory safety in software that grants full control of the machine | Guaranteed by the compiler | Also safe (it has a garbage collector) |
| Native GUI | Tauri, Slint, egui, or Flutter via FFI | Limited options (Fyne, Wails) |
| Agent binary size | Small (a few MB) | Larger, with the runtime embedded |
| References in the same language | RustDesk, IronRDP (full RDP), LinuxPlay, SubnetDesk | NetBird, rdpgw and the Tactical RMM agent, all server-side |
| Development speed | Slower, with a learning curve | Faster |
| ID server and relay | Good (tokio, quinn for QUIC) | Excellent |

Go would be the right choice if OmegaDesk were only a server and a dashboard. But the hard part, and what decides whether it is "better than all the rest", lives in the agent: capturing the screen, encoding video in hardware, streaming with low latency, and injecting keyboard and mouse input. That is where Rust wins by a wide margin.

Writing the server in Rust as well lets us **share the protocol crate** across the agent, the client and the server. That removes duplicated data structures and one more language to maintain.

## Initial layout

A detailed, up-to-date version is in [plan.md](plan.md#rust-workspace).

```
omegadesk/
├── crates/
│   ├── protocol/   # messages (protobuf), end-to-end encryption, handshake
│   ├── capture/    # per-platform screen capture
│   ├── codec/      # hardware H.264/H.265/AV1, with software VP9 as fallback
│   ├── input/      # keyboard and mouse injection
│   ├── transport/  # QUIC + NAT traversal (hole punching) + relay
│   ├── mesh/       # Tailscale-style private network: WireGuard (boringtun), internal IPs and DNS, ACLs
│   └── rdp/        # RDP compatibility (via IronRDP)
├── agent/          # service running on the accessed machine
├── client/         # app for the person connecting (Tauri)
├── server/         # ID, relay, API and session recording
└── web/            # dashboard and browser client (TypeScript + WebRTC/WebCodecs)
```
