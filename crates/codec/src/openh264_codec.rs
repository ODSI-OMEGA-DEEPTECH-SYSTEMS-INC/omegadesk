//! H.264 por software com OpenH264, compilado a partir do código-fonte.

use openh264::OpenH264API;
use openh264::decoder::{Decoder, DecoderConfig};
use openh264::encoder::{
    BitRate, Encoder, EncoderConfig, FrameRate, FrameType, IntraFramePeriod, RateControlMode,
    UsageType,
};
use openh264::formats::{YUVSlices, YUVSource};

use crate::{
    CodecError, DecodedImage, EncodedFrame, EncoderSettings, RawFrame, VideoDecoder, VideoEncoder,
    bgra_to_i420,
};

/// Maior lado aceito, para não estourar memória com dimensões absurdas.
const MAX_DIMENSION: u32 = 8192;

/// Encoder H.264 por software, ajustado para tela em tempo real.
pub struct OpenH264Encoder {
    encoder: Encoder,
}

impl std::fmt::Debug for OpenH264Encoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenH264Encoder").finish_non_exhaustive()
    }
}

impl OpenH264Encoder {
    /// Cria o encoder.
    ///
    /// # Errors
    ///
    /// Retorna erro se o OpenH264 não inicializar.
    pub fn new(settings: EncoderSettings) -> Result<Self, CodecError> {
        let config = EncoderConfig::new()
            .usage_type(UsageType::ScreenContentRealTime)
            .rate_control_mode(RateControlMode::Bitrate)
            .bitrate(BitRate::from_bps(settings.bitrate_bps))
            .max_frame_rate(FrameRate::from_hz(settings.max_fps))
            // Sem pular quadros: latência previsível vale mais que bitrate exato.
            .skip_frames(false)
            // GOP longo; keyframes só quando pedidos (cliente novo ou perda).
            .intra_frame_period(IntraFramePeriod::from_num_frames(0));
        let encoder = Encoder::with_api_config(OpenH264API::from_source(), config)?;
        Ok(Self { encoder })
    }
}

impl VideoEncoder for OpenH264Encoder {
    fn encode(&mut self, frame: RawFrame<'_>) -> Result<Vec<EncodedFrame>, CodecError> {
        if frame.width < 2
            || frame.height < 2
            || frame.width > MAX_DIMENSION
            || frame.height > MAX_DIMENSION
        {
            return Err(CodecError::InvalidDimensions {
                width: frame.width,
                height: frame.height,
            });
        }
        let source_width = frame.width as usize;
        let expected = source_width * frame.height as usize * 4;
        if frame.bgra.len() != expected {
            return Err(CodecError::BufferSize {
                got: frame.bgra.len(),
                expected,
            });
        }

        // H.264 4:2:0 precisa de dimensões pares: descarta a última coluna/linha.
        let width = source_width & !1;
        let height = frame.height as usize & !1;
        let (y, u, v) = bgra_to_i420(frame.bgra, source_width, width, height);

        let yuv = YUVSlices::new((&y, &u, &v), (width, height), (width, width / 2, width / 2));
        let bitstream = self.encoder.encode(&yuv)?;
        let keyframe = matches!(bitstream.frame_type(), FrameType::IDR | FrameType::I);
        if matches!(bitstream.frame_type(), FrameType::Skip | FrameType::Invalid) {
            return Ok(Vec::new());
        }
        let data = bitstream.to_vec();
        if data.is_empty() {
            return Ok(Vec::new());
        }
        Ok(vec![EncodedFrame {
            data,
            keyframe,
            width: u32::try_from(width).unwrap_or(MAX_DIMENSION),
            height: u32::try_from(height).unwrap_or(MAX_DIMENSION),
        }])
    }

    fn request_keyframe(&mut self) {
        self.encoder.force_intra_frame();
    }
}

/// Decoder H.264 por software.
pub struct OpenH264Decoder {
    decoder: Decoder,
    rgba: Vec<u8>,
}

impl std::fmt::Debug for OpenH264Decoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenH264Decoder").finish_non_exhaustive()
    }
}

impl OpenH264Decoder {
    /// Cria o decoder.
    ///
    /// # Errors
    ///
    /// Retorna erro se o OpenH264 não inicializar.
    pub fn new() -> Result<Self, CodecError> {
        let decoder = Decoder::with_api_config(OpenH264API::from_source(), DecoderConfig::new())?;
        Ok(Self {
            decoder,
            rgba: Vec::new(),
        })
    }
}

impl VideoDecoder for OpenH264Decoder {
    fn decode(&mut self, data: &[u8]) -> Result<Option<DecodedImage>, CodecError> {
        let Some(yuv) = self.decoder.decode(data)? else {
            return Ok(None);
        };
        let (width, height) = yuv.dimensions();
        self.rgba.resize(width * height * 4, 0);
        yuv.write_rgba8(&mut self.rgba);

        let xrgb = self
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .map(|px| (u32::from(px[0]) << 16) | (u32::from(px[1]) << 8) | u32::from(px[2]))
            .collect();
        Ok(Some(DecodedImage {
            width: u32::try_from(width).unwrap_or(MAX_DIMENSION),
            height: u32::try_from(height).unwrap_or(MAX_DIMENSION),
            xrgb,
        }))
    }
}
