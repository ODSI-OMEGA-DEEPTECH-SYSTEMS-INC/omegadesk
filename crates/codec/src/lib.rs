//! Codificação e decodificação de vídeo do OmegaDesk.
//!
//! Tudo passa pelas traits [`VideoEncoder`] e [`VideoDecoder`]. A primeira
//! implementação é H.264 por software (OpenH264). Os encoders por hardware
//! (NVENC, QSV, AMF, VAAPI, VideoToolbox) entram como novas implementações,
//! sem mudar quem usa o crate.

mod color;
#[cfg(windows)]
mod media_foundation;
mod openh264_codec;

pub use color::{bgra_to_i420, bgra_to_nv12};
#[cfg(windows)]
pub use media_foundation::MediaFoundationEncoder;
pub use openh264_codec::{OpenH264Decoder, OpenH264Encoder};

/// Erros de codec.
#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    /// Dimensões que o codec não aceita (zero ou grandes demais).
    #[error("dimensões inválidas: {width}x{height}")]
    InvalidDimensions {
        /// Largura recebida.
        width: u32,
        /// Altura recebida.
        height: u32,
    },
    /// O buffer de entrada não tem o tamanho esperado.
    #[error("buffer de {got} bytes; esperado {expected}")]
    BufferSize {
        /// Tamanho recebido.
        got: usize,
        /// Tamanho esperado.
        expected: usize,
    },
    /// Erro do OpenH264.
    #[error("OpenH264: {0}")]
    OpenH264(#[from] openh264::Error),
    /// Não há encoder de hardware nesta máquina.
    #[error("nenhum encoder de hardware disponível")]
    HardwareUnavailable,
    /// Erro da API do sistema (Media Foundation, VA-API…).
    #[error("{0}")]
    Platform(String),
}

/// Quadro de entrada para o encoder, em BGRA sem preenchimento entre linhas.
#[derive(Debug, Clone, Copy)]
pub struct RawFrame<'a> {
    /// Largura em pixels.
    pub width: u32,
    /// Altura em pixels.
    pub height: u32,
    /// `width * height * 4` bytes BGRA.
    pub bgra: &'a [u8],
}

/// Quadro codificado.
#[derive(Debug, Clone)]
pub struct EncodedFrame {
    /// Bitstream (Annex B, no caso do H.264).
    pub data: Vec<u8>,
    /// Quadro que pode ser decodificado sozinho (IDR).
    pub keyframe: bool,
    /// Largura efetivamente codificada (pode ser 1 pixel menor que a entrada).
    pub width: u32,
    /// Altura efetivamente codificada.
    pub height: u32,
}

/// Imagem decodificada, pronta para desenhar.
#[derive(Debug, Clone)]
pub struct DecodedImage {
    /// Largura em pixels.
    pub width: u32,
    /// Altura em pixels.
    pub height: u32,
    /// Um `u32` por pixel no formato `0x00RRGGBB`.
    pub xrgb: Vec<u32>,
}

/// Codificador de vídeo.
pub trait VideoEncoder: Send {
    /// Codifica um quadro e devolve, em ordem, todos os quadros codificados que
    /// ficaram prontos (pode ser nenhum, ou mais de um num encoder com fila).
    /// Quem chama precisa enviar todos: perder um quadro corrompe os seguintes.
    ///
    /// # Errors
    ///
    /// Retorna erro se o quadro for inválido ou o codec falhar.
    fn encode(&mut self, frame: RawFrame<'_>) -> Result<Vec<EncodedFrame>, CodecError>;

    /// Faz o próximo quadro ser um keyframe (quando um cliente entra ou perde dados).
    fn request_keyframe(&mut self);
}

/// Decodificador de vídeo.
pub trait VideoDecoder: Send {
    /// Decodifica um quadro. Devolve `None` se ainda não há imagem completa.
    ///
    /// # Errors
    ///
    /// Retorna erro se o bitstream estiver corrompido.
    fn decode(&mut self, data: &[u8]) -> Result<Option<DecodedImage>, CodecError>;
}

/// Configuração do encoder.
#[derive(Debug, Clone, Copy)]
pub struct EncoderSettings {
    /// Taxa alvo em bits por segundo.
    pub bitrate_bps: u32,
    /// Quadros por segundo máximos.
    pub max_fps: f32,
}

impl Default for EncoderSettings {
    fn default() -> Self {
        Self {
            bitrate_bps: 8_000_000,
            max_fps: 60.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EncoderSettings, OpenH264Decoder, OpenH264Encoder, RawFrame, VideoDecoder, VideoEncoder,
    };

    /// Codifica e decodifica um gradiente e confere que a imagem volta parecida.
    #[test]
    fn round_trip_gradient() {
        let (width, height) = (320_u32, 240_u32);
        let mut bgra = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                let b = u8::try_from(x * 255 / width).unwrap();
                let g = u8::try_from(y * 255 / height).unwrap();
                bgra.extend_from_slice(&[b, g, 128, 255]);
            }
        }

        let mut encoder = OpenH264Encoder::new(EncoderSettings::default()).unwrap();
        let mut decoder = OpenH264Decoder::new().unwrap();

        let frame = RawFrame {
            width,
            height,
            bgra: &bgra,
        };
        let bitstream = encoder.encode(frame).unwrap().remove(0);
        assert!(bitstream.keyframe);

        let image = decoder
            .decode(&bitstream.data)
            .unwrap()
            .expect("imagem decodificada");
        assert_eq!((image.width, image.height), (width, height));

        // Confere o pixel do meio com tolerância de compressão.
        let (col, row) = (width / 2, height / 2);
        let pixel = image.xrgb[(row * width + col) as usize];
        let [_, red, green, blue] = pixel.to_be_bytes();
        let expected_blue = col * 255 / width;
        let expected_green = row * 255 / height;
        assert!(
            u32::from(blue).abs_diff(expected_blue) < 24,
            "azul {blue} vs {expected_blue}"
        );
        assert!(
            u32::from(green).abs_diff(expected_green) < 24,
            "verde {green} vs {expected_green}"
        );
        assert!(u32::from(red).abs_diff(128) < 24, "vermelho {red}");
    }
}
