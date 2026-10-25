//! Captura de tela do OmegaDesk.
//!
//! Hoje há um backend: DXGI Desktop Duplication no Windows, copiando o quadro
//! para a CPU em BGRA. O caminho sem cópia (textura direto para o encoder por
//! hardware) entra junto com os encoders NVENC/QSV/AMF.

use std::time::{Duration, Instant};

#[cfg(windows)]
mod dxgi;

/// Erros de captura.
#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    /// Não há backend de captura para este sistema operacional.
    #[error("captura de tela ainda não suportada neste sistema")]
    Unsupported,
    /// O monitor pedido não existe.
    #[error("monitor {0} não encontrado")]
    DisplayNotFound(u32),
    /// A duplicação foi perdida (troca de resolução, tela de login, UAC…).
    /// Basta abrir o capturador de novo.
    #[error("acesso à tela perdido; reabra a captura")]
    AccessLost,
    /// Erro do sistema operacional.
    #[error("erro do sistema: {0}")]
    Os(String),
}

/// Um quadro capturado, em BGRA 8 bits por canal.
#[derive(Debug, Clone)]
pub struct Frame {
    /// Largura em pixels.
    pub width: u32,
    /// Altura em pixels.
    pub height: u32,
    /// Pixels BGRA, linha a linha, sem preenchimento (`width * 4` bytes por linha).
    pub bgra: Vec<u8>,
    /// Instante da captura.
    pub captured_at: Instant,
}

/// Fonte de quadros de um monitor.
pub trait Capturer: Send {
    /// Espera o próximo quadro até `timeout`.
    ///
    /// Devolve `None` se a tela não mudou nesse intervalo.
    ///
    /// # Errors
    ///
    /// Retorna [`CaptureError::AccessLost`] quando a captura precisa ser
    /// reaberta, ou outro erro do sistema.
    fn next_frame(&mut self, timeout: Duration) -> Result<Option<Frame>, CaptureError>;

    /// Largura e altura do monitor capturado.
    fn size(&self) -> (u32, u32);
}

/// Abre a captura de um monitor (0 é o principal).
///
/// # Errors
///
/// Retorna erro se o monitor não existir ou se o sistema não permitir a captura.
pub fn open_display(index: u32) -> Result<Box<dyn Capturer>, CaptureError> {
    #[cfg(windows)]
    {
        Ok(Box::new(dxgi::DxgiCapturer::open(index)?))
    }
    #[cfg(not(windows))]
    {
        let _ = index;
        Err(CaptureError::Unsupported)
    }
}
