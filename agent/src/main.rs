//! Agente do OmegaDesk: captura a tela, codifica e envia ao cliente; recebe
//! teclado e mouse e injeta na sessão local.
//!
//! Uso (Fase 1, rede local):
//!
//! ```text
//! omegadesk-agent --listen 0.0.0.0:7070
//! ```
//!
//! O agente mostra a impressão digital do certificado; o cliente precisa dela
//! para conectar.

mod capture_loop;

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, bail};
use clap::Parser;
use omegadesk_codec::EncoderSettings;
use omegadesk_input::InputInjector;
use omegadesk_protocol::v1::{
    Capabilities, Envelope, Error as ProtocolError, ErrorCode, HelloAck, Pong, VideoCodec,
    envelope::Payload,
};
use omegadesk_protocol::{PROTOCOL_VERSION, negotiate_codec};
use omegadesk_transport::{Connection, EnvelopeReader, SendStream, send_envelope, server_endpoint};
use tokio::sync::mpsc;
use tracing::{info, warn};

#[derive(Debug, Parser)]
#[command(version, about = "Agente do OmegaDesk")]
struct Args {
    /// Endereço e porta em que o agente aceita conexões.
    #[arg(long, default_value = "0.0.0.0:7070")]
    listen: SocketAddr,

    /// Monitor a compartilhar (0 é o principal).
    #[arg(long, default_value_t = 0)]
    display: u32,

    /// Taxa de vídeo em megabits por segundo.
    #[arg(long, default_value_t = 8)]
    bitrate_mbps: u32,

    /// Quadros por segundo máximos.
    #[arg(long, default_value_t = 60.0)]
    fps: f32,

    /// Não injeta teclado e mouse (só compartilha a tela).
    #[arg(long)]
    no_input: bool,

    /// Nome desta máquina mostrado ao cliente.
    #[arg(long, default_value = "omegadesk-agent")]
    name: String,
}

/// Codecs que este agente sabe codificar, em ordem de preferência.
const ENCODERS: [VideoCodec; 1] = [VideoCodec::H264];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let args = Arc::new(Args::parse());

    let (endpoint, fingerprint) =
        server_endpoint(args.listen).context("não foi possível abrir a porta do agente")?;
    info!("agente ouvindo em {}", endpoint.local_addr()?);
    info!("impressão digital: {fingerprint}");

    // Fase 1: uma sessão por vez. Sessões simultâneas entram com o multiplexador.
    while let Some(incoming) = endpoint.accept().await {
        let connection = match incoming.await {
            Ok(connection) => connection,
            Err(error) => {
                warn!("conexão recusada: {error}");
                continue;
            }
        };
        let remote = connection.remote_address();
        info!("cliente conectado: {remote}");
        match run_session(connection, &args).await {
            Ok(()) => info!("sessão encerrada: {remote}"),
            Err(error) => warn!("sessão com {remote} terminou com erro: {error:#}"),
        }
    }
    Ok(())
}

/// Recebe o `Hello`, escolhe o codec e responde com `HelloAck`.
async fn handshake(
    control_send: &mut SendStream,
    control: &mut EnvelopeReader,
    name: &str,
) -> anyhow::Result<VideoCodec> {
    let Some(Envelope {
        payload: Some(Payload::Hello(hello)),
    }) = control.next().await?
    else {
        bail!("o cliente não começou com Hello");
    };
    if hello.protocol_version != PROTOCOL_VERSION {
        reject(
            control_send,
            ErrorCode::UnsupportedVersion,
            "versão do protocolo diferente",
        )
        .await;
        bail!(
            "cliente usa a versão {} do protocolo",
            hello.protocol_version
        );
    }
    let client_decoders: Vec<VideoCodec> = hello
        .capabilities
        .as_ref()
        .map(|caps| caps.decoders().collect())
        .unwrap_or_default();
    let Some(codec) = negotiate_codec(&ENCODERS, &client_decoders) else {
        reject(
            control_send,
            ErrorCode::NoCommonCodec,
            "nenhum codec em comum",
        )
        .await;
        bail!("nenhum codec em comum com o cliente");
    };
    info!("cliente \"{}\" usa {codec:?}", hello.device_name);

    send_envelope(
        control_send,
        &Envelope {
            payload: Some(Payload::HelloAck(HelloAck {
                protocol_version: PROTOCOL_VERSION,
                device_public_key: Vec::new().into(),
                device_name: name.to_owned(),
                capabilities: Some(Capabilities {
                    decoders: Vec::new(),
                    encoders: ENCODERS.iter().map(|c| *c as i32).collect(),
                }),
                selected_codec: codec as i32,
            })),
        },
    )
    .await?;
    Ok(codec)
}

/// Atende o stream de controle: ping, teclado e mouse.
async fn serve_control(
    control_send: &mut SendStream,
    control: &mut EnvelopeReader,
    mut injector: Option<Box<dyn InputInjector>>,
) -> anyhow::Result<()> {
    while let Some(envelope) = control.next().await? {
        match envelope.payload {
            Some(Payload::Ping(ping)) => {
                let reply = Envelope {
                    payload: Some(Payload::Pong(Pong {
                        sequence: ping.sequence,
                        ping_sent_at_micros: ping.sent_at_micros,
                    })),
                };
                send_envelope(control_send, &reply).await?;
            }
            Some(Payload::KeyEvent(key)) => {
                if let Some(injector) = injector.as_mut()
                    && let Err(error) = injector.key(&key)
                {
                    warn!("tecla não injetada: {error}");
                }
            }
            Some(Payload::PointerEvent(pointer)) => {
                if let Some(injector) = injector.as_mut()
                    && let Err(error) = injector.pointer(&pointer)
                {
                    warn!("mouse não injetado: {error}");
                }
            }
            Some(Payload::Error(error)) => {
                bail!("cliente reportou erro: {}", error.message);
            }
            Some(
                Payload::Hello(_)
                | Payload::HelloAck(_)
                | Payload::Pong(_)
                | Payload::VideoFrame(_),
            )
            | None => {}
        }
    }
    Ok(())
}

async fn run_session(connection: Connection, args: &Args) -> anyhow::Result<()> {
    let (mut control_send, control_recv) = connection.accept_bi().await?;
    let mut control = EnvelopeReader::new(control_recv);
    handshake(&mut control_send, &mut control, &args.name).await?;

    // Vídeo num stream próprio, para não atrasar as mensagens de controle.
    let mut video_send = connection.open_uni().await?;
    let stop = Arc::new(AtomicBool::new(false));
    let keyframe_requested = Arc::new(AtomicBool::new(true));
    let (frame_tx, mut frame_rx) = mpsc::channel(2);

    let capture = {
        let settings = EncoderSettings {
            bitrate_bps: args.bitrate_mbps.saturating_mul(1_000_000),
            max_fps: args.fps,
        };
        let stop = stop.clone();
        let keyframe_requested = keyframe_requested.clone();
        let display = args.display;
        std::thread::Builder::new()
            .name("captura".into())
            .spawn(move || {
                capture_loop::run(display, settings, &frame_tx, &stop, &keyframe_requested);
            })?
    };

    let video = tokio::spawn(async move {
        while let Some(frame) = frame_rx.recv().await {
            let envelope = Envelope {
                payload: Some(Payload::VideoFrame(frame)),
            };
            if send_envelope(&mut video_send, &envelope).await.is_err() {
                break;
            }
        }
    });

    let injector = if args.no_input {
        None
    } else {
        match omegadesk_input::open() {
            Ok(injector) => Some(injector),
            Err(error) => {
                warn!("sem controle remoto de teclado e mouse: {error}");
                None
            }
        }
    };

    let result = serve_control(&mut control_send, &mut control, injector).await;

    stop.store(true, Ordering::Relaxed);
    video.abort();
    if capture.join().is_err() {
        warn!("a thread de captura terminou com pânico");
    }
    result
}

async fn reject(stream: &mut SendStream, code: ErrorCode, message: &str) {
    let envelope = Envelope {
        payload: Some(Payload::Error(ProtocolError {
            code: code as i32,
            message: message.to_owned(),
        })),
    };
    let _ = send_envelope(stream, &envelope).await;
}
