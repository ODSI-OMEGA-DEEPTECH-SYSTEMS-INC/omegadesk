//! Conexão com o agente, numa thread com runtime tokio, e decodificação numa
//! thread própria.

use std::net::SocketAddr;
use std::sync::mpsc as std_mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, bail};
use omegadesk_codec::{OpenH264Decoder, VideoDecoder};
use omegadesk_protocol::PROTOCOL_VERSION;
use omegadesk_protocol::v1::{
    Capabilities, Envelope, Hello, Ping, VideoCodec, VideoFrame, envelope::Payload,
};
use omegadesk_transport::{EnvelopeReader, SendStream, connect, send_envelope};
use tokio::sync::mpsc;
use tracing::{info, warn};
use winit::event_loop::EventLoopProxy;

use crate::stats::{LatestFrame, Shared};
use crate::window::UserEvent;

/// Intervalo entre pings de medição de latência.
const PING_INTERVAL: Duration = Duration::from_millis(500);

fn unix_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_micros()).unwrap_or(u64::MAX))
}

/// Inicia a conexão em segundo plano. Devolve o canal para enviar eventos de
/// teclado e mouse ao agente.
pub(crate) fn start(
    address: SocketAddr,
    fingerprint: String,
    shared: Shared,
    proxy: EventLoopProxy<UserEvent>,
) -> mpsc::UnboundedSender<Envelope> {
    let (input_tx, input_rx) = mpsc::unbounded_channel();
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                shared.set_status(format!("erro: {error}"));
                let _ = proxy.send_event(UserEvent::Disconnected);
                return;
            }
        };
        let result = runtime.block_on(run(
            address,
            &fingerprint,
            shared.clone(),
            proxy.clone(),
            input_rx,
        ));
        let status = match result {
            Ok(()) => "desconectado".to_owned(),
            Err(error) => format!("erro: {error:#}"),
        };
        warn!("{status}");
        shared.set_status(status);
        let _ = proxy.send_event(UserEvent::Disconnected);
    });
    input_tx
}

async fn run(
    address: SocketAddr,
    fingerprint: &str,
    shared: Shared,
    proxy: EventLoopProxy<UserEvent>,
    mut input_rx: mpsc::UnboundedReceiver<Envelope>,
) -> anyhow::Result<()> {
    shared.set_status(format!("conectando a {address}…"));
    let _ = proxy.send_event(UserEvent::Status);
    let (_endpoint, connection) = connect(address, fingerprint)
        .await
        .context("não foi possível conectar ao agente")?;

    let (mut control_send, control_recv) = connection.open_bi().await?;
    let mut control = EnvelopeReader::new(control_recv);
    send_envelope(
        &mut control_send,
        &Envelope {
            payload: Some(Payload::Hello(Hello {
                protocol_version: PROTOCOL_VERSION,
                device_public_key: Vec::new().into(),
                device_name: "omegadesk-client".to_owned(),
                capabilities: Some(Capabilities {
                    decoders: vec![VideoCodec::H264 as i32],
                    encoders: Vec::new(),
                }),
            })),
        },
    )
    .await?;

    let ack = match control.next().await? {
        Some(Envelope {
            payload: Some(Payload::HelloAck(ack)),
        }) => ack,
        Some(Envelope {
            payload: Some(Payload::Error(error)),
        }) => bail!("o agente recusou: {}", error.message),
        _ => bail!("resposta inesperada do agente"),
    };
    info!("conectado a \"{}\"", ack.device_name);
    shared.set_status(format!("conectado a {}", ack.device_name));
    let _ = proxy.send_event(UserEvent::Status);

    let video_recv = connection.accept_uni().await?;
    let decode_tx = spawn_decoder(shared.clone(), proxy.clone());

    let video = tokio::spawn(async move {
        let mut reader = EnvelopeReader::new(video_recv);
        loop {
            match reader.next().await {
                Ok(Some(envelope)) => {
                    if let Some(Payload::VideoFrame(frame)) = envelope.payload
                        && decode_tx.send(frame).is_err()
                    {
                        warn!("decoder parou; encerrando o vídeo");
                        break;
                    }
                }
                Ok(None) => {
                    warn!("o agente encerrou o stream de vídeo");
                    break;
                }
                Err(error) => {
                    warn!("erro no stream de vídeo: {error}");
                    break;
                }
            }
        }
    });

    let control_task = control_loop(&mut control_send, &mut control, &mut input_rx, &shared);
    let result = tokio::select! {
        result = control_task => result,
        _ = video => Ok(()),
    };
    connection.close(0u32.into(), b"fim");
    result
}

/// Envia pings e os eventos de entrada; recebe os pongs.
async fn control_loop(
    send: &mut SendStream,
    control: &mut EnvelopeReader,
    input_rx: &mut mpsc::UnboundedReceiver<Envelope>,
    shared: &Shared,
) -> anyhow::Result<()> {
    let started = Instant::now();
    let mut ticker = tokio::time::interval(PING_INTERVAL);
    let mut sequence = 0u64;
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                sequence += 1;
                let ping = Envelope {
                    payload: Some(Payload::Ping(Ping {
                        sequence,
                        sent_at_micros: u64::try_from(started.elapsed().as_micros())
                            .unwrap_or(u64::MAX),
                    })),
                };
                send_envelope(send, &ping).await?;
            }
            event = input_rx.recv() => {
                let Some(event) = event else { return Ok(()) };
                send_envelope(send, &event).await?;
            }
            envelope = control.next() => {
                match envelope? {
                    Some(Envelope { payload: Some(Payload::Pong(pong)) }) => {
                        let now = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
                        shared.set_rtt(Duration::from_micros(
                            now.saturating_sub(pong.ping_sent_at_micros),
                        ));
                    }
                    Some(_) => {}
                    None => return Ok(()),
                }
            }
        }
    }
}

/// Thread de decodificação: recebe quadros, decodifica e publica o mais recente.
fn spawn_decoder(shared: Shared, proxy: EventLoopProxy<UserEvent>) -> std_mpsc::Sender<VideoFrame> {
    let (tx, rx) = std_mpsc::channel::<VideoFrame>();
    std::thread::spawn(move || {
        let mut decoder = match OpenH264Decoder::new() {
            Ok(decoder) => decoder,
            Err(error) => {
                shared.set_status(format!("decoder indisponível: {error}"));
                return;
            }
        };
        let mut logged_clock_skew = false;
        while let Ok(frame) = rx.recv() {
            let start = Instant::now();
            match decoder.decode(&frame.data) {
                Ok(Some(image)) => {
                    shared.publish(LatestFrame {
                        image,
                        host_processing: Duration::from_micros(u64::from(
                            frame.host_processing_micros,
                        )),
                        decode: start.elapsed(),
                        decoded_at: Instant::now(),
                    });
                    if proxy.send_event(UserEvent::Frame).is_err() {
                        break;
                    }
                }
                Ok(None) => {}
                Err(error) => warn!("quadro {} não decodificado: {error}", frame.sequence),
            }
            if !logged_clock_skew {
                let skew = unix_micros().abs_diff(frame.captured_at_micros);
                info!(
                    "diferença de relógio host/cliente + rede: {} ms",
                    skew / 1000
                );
                logged_clock_skew = true;
            }
        }
    });
    tx
}
