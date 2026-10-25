//! Laço de captura e codificação, numa thread própria (as APIs são bloqueantes).

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use omegadesk_capture::{CaptureError, Capturer, Frame};
use omegadesk_codec::{EncoderSettings, OpenH264Encoder, RawFrame, VideoEncoder};
use omegadesk_protocol::v1::{VideoCodec, VideoFrame};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Quanto esperar por um quadro novo antes de checar se a sessão acabou.
const POLL: Duration = Duration::from_millis(100);

/// Pausa antes de reabrir a captura depois de um erro.
const RETRY: Duration = Duration::from_millis(500);

fn unix_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_micros()).unwrap_or(u64::MAX))
}

fn open(monitor: u32) -> Option<Box<dyn Capturer>> {
    match omegadesk_capture::open_display(monitor) {
        Ok(capturer) => {
            let (width, height) = capturer.size();
            info!("capturando o monitor {monitor} em {width}x{height}");
            Some(capturer)
        }
        Err(error) => {
            warn!("captura indisponível: {error}");
            None
        }
    }
}

/// Captura, codifica e envia quadros até `stop` ou até o canal fechar.
pub(crate) fn run(
    display: u32,
    settings: EncoderSettings,
    frames: &mpsc::Sender<VideoFrame>,
    stop: &AtomicBool,
    keyframe_requested: &AtomicBool,
) {
    let mut encoder = match OpenH264Encoder::new(settings) {
        Ok(encoder) => encoder,
        Err(error) => {
            warn!("encoder indisponível: {error}");
            return;
        }
    };
    let mut capturer: Option<Box<dyn Capturer>> = None;
    let mut last: Option<Frame> = None;
    let mut sequence = 0u64;
    let mut timing = Timing::default();

    while !stop.load(Ordering::Relaxed) {
        let Some(active) = capturer.as_mut() else {
            capturer = open(display);
            if capturer.is_none() {
                std::thread::sleep(RETRY);
            }
            continue;
        };

        let force_keyframe = keyframe_requested.swap(false, Ordering::Relaxed);
        if force_keyframe {
            encoder.request_keyframe();
        }

        let frame = match active.next_frame(POLL) {
            Ok(Some(frame)) => frame,
            // Tela parada: só reenvia se o cliente pediu keyframe.
            Ok(None) => match (&last, force_keyframe) {
                (Some(previous), true) => previous.clone(),
                _ => continue,
            },
            Err(CaptureError::AccessLost) => {
                debug!("acesso à tela perdido; reabrindo");
                capturer = None;
                keyframe_requested.store(true, Ordering::Relaxed);
                continue;
            }
            Err(error) => {
                warn!("erro de captura: {error}");
                capturer = None;
                std::thread::sleep(RETRY);
                continue;
            }
        };

        let encode_start = Instant::now();
        let encoded = encoder.encode(RawFrame {
            width: frame.width,
            height: frame.height,
            bgra: &frame.bgra,
        });
        let host_processing = frame.captured_at.elapsed();
        timing.record(encode_start - frame.captured_at, encode_start.elapsed());
        let encoded = match encoded {
            Ok(Some(encoded)) => encoded,
            Ok(None) => continue,
            Err(error) => {
                warn!("erro de codificação: {error}");
                continue;
            }
        };

        sequence += 1;
        let video_frame = VideoFrame {
            sequence,
            captured_at_micros: unix_micros(),
            host_processing_micros: u32::try_from(host_processing.as_micros()).unwrap_or(u32::MAX),
            codec: VideoCodec::H264 as i32,
            keyframe: encoded.keyframe,
            display_index: display,
            width: encoded.width,
            height: encoded.height,
            data: encoded.data.into(),
        };
        last = Some(frame);
        if frames.blocking_send(video_frame).is_err() {
            break;
        }
    }
}

/// Tempos por etapa, registrados no log a cada [`TIMING_PERIOD`].
#[derive(Debug, Default)]
struct Timing {
    frames: u32,
    capture: Duration,
    encode: Duration,
    since: Option<Instant>,
}

/// Intervalo entre os registros de tempo no log.
const TIMING_PERIOD: Duration = Duration::from_secs(5);

impl Timing {
    fn record(&mut self, capture: Duration, encode: Duration) {
        let since = *self.since.get_or_insert_with(Instant::now);
        self.frames += 1;
        self.capture += capture;
        self.encode += encode;
        let elapsed = since.elapsed();
        if elapsed >= TIMING_PERIOD {
            let ms = |d: Duration| d.as_secs_f64() * 1000.0 / f64::from(self.frames);
            info!(
                "{:.1} quadros/s · cópia da tela {:.1} ms · codificação {:.1} ms",
                f64::from(self.frames) / elapsed.as_secs_f64(),
                ms(self.capture),
                ms(self.encode),
            );
            *self = Self::default();
        }
    }
}
