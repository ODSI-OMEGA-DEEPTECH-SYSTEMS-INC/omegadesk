//! Injeção de teclado e mouse do OmegaDesk.
//!
//! As teclas chegam como código de uso USB HID (independente do layout) e são
//! convertidas em scancode do teclado físico na máquina remota. Assim o
//! layout configurado lá (ABNT2, US, …) decide o caractere, como num teclado
//! de verdade.

mod hid;
#[cfg(windows)]
mod windows_injector;

pub use hid::{Scancode, hid_to_scancode};
use omegadesk_protocol::v1::{KeyEvent, PointerEvent};

/// Botões do ponteiro, como bits de `PointerEvent::buttons`.
pub mod buttons {
    /// Botão esquerdo.
    pub const LEFT: u32 = 1;
    /// Botão direito.
    pub const RIGHT: u32 = 1 << 1;
    /// Botão do meio.
    pub const MIDDLE: u32 = 1 << 2;
    /// Botão lateral "voltar".
    pub const BACK: u32 = 1 << 3;
    /// Botão lateral "avançar".
    pub const FORWARD: u32 = 1 << 4;
}

/// Uma unidade de rolagem (um "clique" da roda), igual ao `WHEEL_DELTA` do Windows.
pub const WHEEL_STEP: i32 = 120;

/// Erros de injeção.
#[derive(Debug, thiserror::Error)]
pub enum InputError {
    /// Não há injeção para este sistema operacional.
    #[error("injeção de entrada ainda não suportada neste sistema")]
    Unsupported,
    /// O sistema recusou o evento (por exemplo, janela com privilégio maior).
    #[error("o sistema recusou o evento de entrada")]
    Rejected,
}

/// Injeta eventos de entrada na sessão local.
pub trait InputInjector: Send {
    /// Pressiona ou solta uma tecla.
    ///
    /// # Errors
    ///
    /// Retorna erro se o sistema recusar o evento.
    fn key(&mut self, event: &KeyEvent) -> Result<(), InputError>;

    /// Move o ponteiro, aperta/solta botões e rola.
    ///
    /// # Errors
    ///
    /// Retorna erro se o sistema recusar o evento.
    fn pointer(&mut self, event: &PointerEvent) -> Result<(), InputError>;
}

/// Abre o injetor do sistema operacional atual.
///
/// # Errors
///
/// Retorna [`InputError::Unsupported`] fora do Windows, por enquanto.
pub fn open() -> Result<Box<dyn InputInjector>, InputError> {
    #[cfg(windows)]
    {
        Ok(Box::new(windows_injector::WindowsInjector::default()))
    }
    #[cfg(not(windows))]
    {
        Err(InputError::Unsupported)
    }
}
