//! Transporte QUIC do OmegaDesk.
//!
//! Nesta fase o agente gera um certificado autoassinado e mostra a impressão
//! digital (SHA-256) dele. O cliente só aceita a conexão se o certificado
//! tiver exatamente essa impressão digital. Na Fase 2 a identidade passa a
//! vir das chaves do dispositivo (`omegadesk-identity`) e do coordenador.

mod pinned;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use bytes::BytesMut;
use omegadesk_protocol::v1::Envelope;
use omegadesk_protocol::{FrameDecoder, FramingError, encode_frame};
use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use sha2::{Digest, Sha256};

pub use quinn::{Connection, Endpoint, RecvStream, SendStream};

/// Protocolo anunciado no ALPN.
pub const ALPN: &[u8] = b"omegadesk/1";

/// Porta padrão do agente.
pub const DEFAULT_PORT: u16 = 7070;

/// Erros de transporte.
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    /// Falha ao gerar o certificado.
    #[error("certificado: {0}")]
    Certificate(#[from] rcgen::Error),
    /// Configuração TLS inválida.
    #[error("TLS: {0}")]
    Tls(#[from] rustls::Error),
    /// Configuração QUIC inválida.
    #[error("QUIC: {0}")]
    QuicConfig(String),
    /// Falha de E/S ao abrir o socket.
    #[error("socket: {0}")]
    Io(#[from] std::io::Error),
    /// Falha ao iniciar a conexão.
    #[error("conexão: {0}")]
    Connect(#[from] quinn::ConnectError),
    /// A conexão caiu ou foi recusada.
    #[error("conexão: {0}")]
    Connection(#[from] quinn::ConnectionError),
    /// Falha ao escrever no stream.
    #[error("envio: {0}")]
    Write(#[from] quinn::WriteError),
    /// Falha ao ler do stream.
    #[error("recebimento: {0}")]
    Read(#[from] quinn::ReadError),
    /// Mensagem malformada.
    #[error(transparent)]
    Framing(#[from] FramingError),
    /// A impressão digital informada não é válida.
    #[error("impressão digital inválida")]
    InvalidFingerprint,
}

/// Impressão digital SHA-256 de um certificado, em hexadecimal minúsculo
/// com `:` a cada byte (o formato que o agente mostra).
#[must_use]
pub fn fingerprint(certificate: &[u8]) -> String {
    let digest = Sha256::digest(certificate);
    data_encoding::HEXLOWER
        .encode(&digest)
        .as_bytes()
        .chunks(2)
        .filter_map(|pair| std::str::from_utf8(pair).ok())
        .collect::<Vec<_>>()
        .join(":")
}

fn parse_fingerprint(text: &str) -> Result<[u8; 32], TransportError> {
    let hex: String = text
        .chars()
        .filter(char::is_ascii_hexdigit)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    let bytes = data_encoding::HEXLOWER
        .decode(hex.as_bytes())
        .map_err(|_| TransportError::InvalidFingerprint)?;
    bytes
        .try_into()
        .map_err(|_| TransportError::InvalidFingerprint)
}

fn crypto_provider() -> Arc<rustls::crypto::CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

fn transport_config() -> quinn::TransportConfig {
    let mut config = quinn::TransportConfig::default();
    config.keep_alive_interval(Some(Duration::from_secs(2)));
    // 30 s, o padrão do quinn: com 10 s, perdas em rajada derrubavam a conexão.
    if let Ok(timeout) = Duration::from_secs(30).try_into() {
        config.max_idle_timeout(Some(timeout));
    }
    // Janelas pequenas de propósito: o que não cabe aqui fica no agente, que
    // descarta quadros e pede keyframe, em vez de acumular segundos de vídeo
    // atrasado no buffer do QUIC. 4 MiB cabem um keyframe 4K.
    config.stream_receive_window(quinn::VarInt::from_u32(4 * 1024 * 1024));
    config.receive_window(quinn::VarInt::from_u32(8 * 1024 * 1024));
    config.send_window(4 * 1024 * 1024);
    config
}

/// Endpoint do agente com um certificado autoassinado novo.
///
/// Devolve o endpoint e a impressão digital que o cliente precisa informar.
///
/// # Errors
///
/// Retorna erro se o certificado não puder ser gerado ou o socket aberto.
pub fn server_endpoint(bind: SocketAddr) -> Result<(Endpoint, String), TransportError> {
    let certified = rcgen::generate_simple_self_signed(vec!["omegadesk".to_owned()])?;
    let certificate: CertificateDer<'static> = certified.cert.der().clone();
    let fingerprint = fingerprint(&certificate);
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
        certified.signing_key.serialize_der(),
    ));

    let mut tls = rustls::ServerConfig::builder_with_provider(crypto_provider())
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .with_no_client_auth()
        .with_single_cert(vec![certificate], key)?;
    tls.alpn_protocols = vec![ALPN.to_vec()];

    let crypto =
        QuicServerConfig::try_from(tls).map_err(|e| TransportError::QuicConfig(e.to_string()))?;
    let mut config = quinn::ServerConfig::with_crypto(Arc::new(crypto));
    config.transport_config(Arc::new(transport_config()));

    Ok((Endpoint::server(config, bind)?, fingerprint))
}

/// Conecta ao agente em `address`, aceitando só o certificado com a
/// impressão digital `expected_fingerprint`.
///
/// # Errors
///
/// Retorna erro se a impressão digital for inválida, se a conexão falhar ou
/// se o certificado do agente não bater.
pub async fn connect(
    address: SocketAddr,
    expected_fingerprint: &str,
) -> Result<(Endpoint, Connection), TransportError> {
    let expected = parse_fingerprint(expected_fingerprint)?;
    let provider = crypto_provider();

    let mut tls = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(pinned::PinnedVerifier::new(expected, provider)))
        .with_no_client_auth();
    tls.alpn_protocols = vec![ALPN.to_vec()];

    let crypto =
        QuicClientConfig::try_from(tls).map_err(|e| TransportError::QuicConfig(e.to_string()))?;
    let mut config = quinn::ClientConfig::new(Arc::new(crypto));
    config.transport_config(Arc::new(transport_config()));

    let bind: SocketAddr = if address.is_ipv6() {
        (std::net::Ipv6Addr::UNSPECIFIED, 0).into()
    } else {
        (std::net::Ipv4Addr::UNSPECIFIED, 0).into()
    };
    let mut endpoint = Endpoint::client(bind)?;
    endpoint.set_default_client_config(config);
    let connection = endpoint.connect(address, "omegadesk")?.await?;
    Ok((endpoint, connection))
}

/// Envia um `Envelope` enquadrado num stream.
///
/// # Errors
///
/// Retorna erro se a mensagem for grande demais ou o stream fechar.
pub async fn send_envelope(
    stream: &mut SendStream,
    envelope: &Envelope,
) -> Result<(), TransportError> {
    let mut buffer = BytesMut::new();
    encode_frame(envelope, &mut buffer)?;
    stream.write_all(&buffer).await?;
    Ok(())
}

/// Lê `Envelope`s enquadrados de um stream.
#[derive(Debug)]
pub struct EnvelopeReader {
    stream: RecvStream,
    decoder: FrameDecoder,
    chunk: Vec<u8>,
}

impl EnvelopeReader {
    /// Cria o leitor sobre um stream de recebimento.
    #[must_use]
    pub fn new(stream: RecvStream) -> Self {
        Self {
            stream,
            decoder: FrameDecoder::new(),
            chunk: vec![0; 64 * 1024],
        }
    }

    /// Próximo `Envelope`, ou `None` quando o outro lado fecha o stream.
    ///
    /// # Errors
    ///
    /// Retorna erro se a leitura falhar ou a mensagem for inválida.
    pub async fn next(&mut self) -> Result<Option<Envelope>, TransportError> {
        loop {
            if let Some(envelope) = self.decoder.next_frame()? {
                return Ok(Some(envelope));
            }
            match self.stream.read(&mut self.chunk).await? {
                Some(read) => self.decoder.extend(&self.chunk[..read]),
                None => return Ok(None),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddr};

    use omegadesk_protocol::v1::{Envelope, Ping, envelope::Payload};

    use super::{EnvelopeReader, TransportError, connect, send_envelope, server_endpoint};

    fn localhost() -> SocketAddr {
        (Ipv4Addr::LOCALHOST, 0).into()
    }

    #[tokio::test]
    async fn round_trip_over_quic() {
        let (server, fingerprint) = server_endpoint(localhost()).unwrap();
        let address = server.local_addr().unwrap();

        let accept = tokio::spawn(async move {
            let connection = server.accept().await.unwrap().await.unwrap();
            let (mut send, recv) = connection.accept_bi().await.unwrap();
            let mut reader = EnvelopeReader::new(recv);
            let envelope = reader.next().await.unwrap().unwrap();
            send_envelope(&mut send, &envelope).await.unwrap();
            send.finish().unwrap();
            connection.closed().await;
        });

        let (_endpoint, connection) = connect(address, &fingerprint).await.unwrap();
        let (mut send, recv) = connection.open_bi().await.unwrap();
        let ping = Envelope {
            payload: Some(Payload::Ping(Ping {
                sequence: 42,
                sent_at_micros: 7,
            })),
        };
        send_envelope(&mut send, &ping).await.unwrap();

        let mut reader = EnvelopeReader::new(recv);
        assert_eq!(reader.next().await.unwrap(), Some(ping));
        connection.close(0u32.into(), b"fim");
        accept.await.unwrap();
    }

    #[tokio::test]
    async fn rejects_wrong_fingerprint() {
        let (server, _) = server_endpoint(localhost()).unwrap();
        let address = server.local_addr().unwrap();
        tokio::spawn(async move {
            if let Some(incoming) = server.accept().await {
                let _ = incoming.await;
            }
        });

        let wrong = "00".repeat(32);
        let result = connect(address, &wrong).await;
        assert!(matches!(result, Err(TransportError::Connection(_))));
    }

    #[tokio::test]
    async fn rejects_malformed_fingerprint() {
        let result = connect(localhost(), "abc").await;
        assert!(matches!(result, Err(TransportError::InvalidFingerprint)));
    }
}
