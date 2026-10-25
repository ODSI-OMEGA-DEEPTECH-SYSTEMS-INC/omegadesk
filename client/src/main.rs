//! Cliente do OmegaDesk: mostra a tela do agente e envia teclado e mouse.
//!
//! Uso (Fase 1, rede local):
//!
//! ```text
//! omegadesk-client 192.168.0.10:7070 --fingerprint ab:cd:…
//! ```
//!
//! A barra de título mostra a latência: tempo no host, rede (metade do ping),
//! decodificação e desenho.

mod keymap;
mod network;
mod stats;
mod window;

use std::net::SocketAddr;

use anyhow::Context;
use clap::Parser;
use winit::event_loop::EventLoop;

#[derive(Debug, Parser)]
#[command(version, about = "Cliente do OmegaDesk")]
struct Args {
    /// Endereço do agente, por exemplo `192.168.0.10:7070`.
    address: SocketAddr,

    /// Impressão digital mostrada pelo agente ao iniciar.
    #[arg(long)]
    fingerprint: String,

    /// Só assiste: não envia teclado nem mouse.
    #[arg(long)]
    view_only: bool,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let args = Args::parse();

    let event_loop = EventLoop::<window::UserEvent>::with_user_event()
        .build()
        .context("não foi possível criar a janela")?;
    let shared = stats::Shared::default();
    let input = network::start(
        args.address,
        args.fingerprint,
        shared.clone(),
        event_loop.create_proxy(),
    );

    let mut app = window::App::new(shared, input, args.view_only);
    event_loop.run_app(&mut app)?;
    app.into_result()
}
