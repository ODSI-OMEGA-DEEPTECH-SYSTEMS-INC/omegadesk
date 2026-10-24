//! Protocolo de sessão do OmegaDesk.
//!
//! As mensagens são definidas em `proto/omegadesk/v1/session.proto` e geradas
//! com `prost`. Este crate também define o enquadramento usado nos streams:
//! cada [`Envelope`](v1::Envelope) é precedido do tamanho em varint.

mod framing;

pub use framing::{FrameDecoder, FramingError, MAX_FRAME_LEN, encode_frame};

/// Versão atual do protocolo, enviada em `Hello` e `HelloAck`.
pub const PROTOCOL_VERSION: u32 = 1;

/// Mensagens da versão 1 do protocolo.
#[allow(
    missing_docs,
    unreachable_pub,
    clippy::pedantic,
    clippy::wildcard_enum_match_arm
)]
pub mod v1 {
    include!(concat!(env!("OUT_DIR"), "/omegadesk.v1.rs"));
}

/// Escolhe o codec da sessão: o primeiro da lista de preferência do host que o
/// cliente consegue decodificar.
#[must_use]
pub fn negotiate_codec(
    host_encoders: &[v1::VideoCodec],
    client_decoders: &[v1::VideoCodec],
) -> Option<v1::VideoCodec> {
    host_encoders
        .iter()
        .copied()
        .filter(|codec| *codec != v1::VideoCodec::Unspecified)
        .find(|codec| client_decoders.contains(codec))
}

#[cfg(test)]
mod tests {
    use super::negotiate_codec;
    use super::v1::VideoCodec;

    #[test]
    fn negotiate_follows_host_preference() {
        let host = [VideoCodec::Av1, VideoCodec::H265, VideoCodec::H264];
        let client = [VideoCodec::H264, VideoCodec::H265];
        assert_eq!(negotiate_codec(&host, &client), Some(VideoCodec::H265));
    }

    #[test]
    fn negotiate_without_common_codec() {
        let host = [VideoCodec::Av1];
        let client = [VideoCodec::H264];
        assert_eq!(negotiate_codec(&host, &client), None);
    }

    #[test]
    fn negotiate_ignores_unspecified() {
        let host = [VideoCodec::Unspecified, VideoCodec::H264];
        let client = [VideoCodec::Unspecified, VideoCodec::H264];
        assert_eq!(negotiate_codec(&host, &client), Some(VideoCodec::H264));
    }
}
