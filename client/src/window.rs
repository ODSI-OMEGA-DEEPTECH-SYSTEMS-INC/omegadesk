//! Janela do cliente: desenha o quadro mais recente e captura teclado e mouse.

use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::Duration;

use anyhow::anyhow;
use omegadesk_input::{WHEEL_STEP, buttons};
use omegadesk_protocol::v1::{Envelope, KeyEvent, PointerEvent, envelope::Payload};
use softbuffer::{Context, Surface};
use tokio::sync::mpsc;
use tracing::warn;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::keymap::keycode_to_hid;
use crate::stats::{LatestFrame, Meter, Shared};

/// Intervalo de atualização da barra de título.
const TITLE_PERIOD: Duration = Duration::from_millis(500);

/// Eventos enviados à janela pelas outras threads.
#[derive(Debug, Clone, Copy)]
pub(crate) enum UserEvent {
    /// Há um quadro novo para desenhar.
    Frame,
    /// O texto de status mudou.
    Status,
    /// A conexão terminou.
    Disconnected,
}

struct Graphics {
    window: Rc<Window>,
    surface: Surface<Rc<Window>, Rc<Window>>,
}

pub(crate) struct App {
    shared: Shared,
    input: mpsc::UnboundedSender<Envelope>,
    view_only: bool,
    graphics: Option<Graphics>,
    current: Option<LatestFrame>,
    meter: Meter,
    cursor: (f64, f64),
    pressed_buttons: u32,
    error: Option<anyhow::Error>,
}

impl App {
    pub(crate) fn new(
        shared: Shared,
        input: mpsc::UnboundedSender<Envelope>,
        view_only: bool,
    ) -> Self {
        Self {
            shared,
            input,
            view_only,
            graphics: None,
            current: None,
            meter: Meter::default(),
            cursor: (0.0, 0.0),
            pressed_buttons: 0,
            error: None,
        }
    }

    pub(crate) fn into_result(self) -> anyhow::Result<()> {
        self.error.map_or(Ok(()), Err)
    }

    fn send(&self, payload: Payload) {
        if !self.view_only {
            let _ = self.input.send(Envelope {
                payload: Some(payload),
            });
        }
    }

    fn send_pointer(&self, wheel_x: i32, wheel_y: i32) {
        let Some(graphics) = &self.graphics else {
            return;
        };
        let size = graphics.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        let normalize = |position: f64, extent: u32| -> u32 {
            let ratio = (position / f64::from(extent)).clamp(0.0, 1.0);
            // 0.0..=1.0 vezes 65535 cabe em u32 sem perda de sinal.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let scaled = (ratio * 65_535.0).round() as u32;
            scaled
        };
        self.send(Payload::PointerEvent(PointerEvent {
            display_index: 0,
            x: normalize(self.cursor.0, size.width),
            y: normalize(self.cursor.1, size.height),
            buttons: self.pressed_buttons,
            wheel_x,
            wheel_y,
        }));
    }

    fn update_title(&self, extra: &str) {
        if let Some(graphics) = &self.graphics {
            let status = self.shared.status();
            graphics
                .window
                .set_title(&format!("OmegaDesk — {status}{extra}"));
        }
    }

    fn redraw(&mut self) {
        if let Some(latest) = self.shared.take_latest() {
            self.current = Some(latest);
        }
        let (Some(graphics), Some(frame)) = (&mut self.graphics, &self.current) else {
            return;
        };
        let size = graphics.window.inner_size();
        let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return;
        };
        if let Err(error) = graphics.surface.resize(width, height) {
            warn!("não foi possível redimensionar a superfície: {error}");
            return;
        }
        let mut buffer = match graphics.surface.buffer_mut() {
            Ok(buffer) => buffer,
            Err(error) => {
                warn!("superfície indisponível: {error}");
                return;
            }
        };
        scale_nearest(&frame.image, &mut buffer, size.width, size.height);
        if let Err(error) = buffer.present() {
            warn!("falha ao apresentar o quadro: {error}");
            return;
        }

        self.meter.record(
            frame.host_processing,
            frame.decode,
            frame.decoded_at.elapsed(),
        );
        if let Some(summary) = self.meter.summary(TITLE_PERIOD, self.shared.rtt()) {
            let ms = |d: Duration| d.as_secs_f64() * 1000.0;
            let image = &frame.image;
            self.update_title(&format!(
                " — {}x{} {:.0} fps — host {:.1} ms · rede {:.1} ms · decod {:.1} ms · desenho {:.1} ms · total {:.1} ms",
                image.width,
                image.height,
                summary.fps,
                ms(summary.host),
                ms(summary.network),
                ms(summary.decode),
                ms(summary.render),
                ms(summary.total()),
            ));
        }
    }
}

/// Redimensiona a imagem para o tamanho da janela (vizinho mais próximo).
fn scale_nearest(
    image: &omegadesk_codec::DecodedImage,
    target: &mut [u32],
    width: u32,
    height: u32,
) {
    if image.width == 0 || image.height == 0 {
        return;
    }
    let (src_w, src_h) = (image.width as usize, image.height as usize);
    let (dst_w, dst_h) = (width as usize, height as usize);
    if (src_w, src_h) == (dst_w, dst_h) {
        target.copy_from_slice(&image.xrgb[..dst_w * dst_h]);
        return;
    }
    let columns: Vec<usize> = (0..dst_w).map(|x| x * src_w / dst_w).collect();
    for (y, row) in target.chunks_exact_mut(dst_w).enumerate().take(dst_h) {
        let src_row = &image.xrgb[(y * src_h / dst_h) * src_w..][..src_w];
        for (pixel, &column) in row.iter_mut().zip(&columns) {
            *pixel = src_row[column];
        }
    }
}

fn button_bit(button: MouseButton) -> u32 {
    match button {
        MouseButton::Left => buttons::LEFT,
        MouseButton::Right => buttons::RIGHT,
        MouseButton::Middle => buttons::MIDDLE,
        MouseButton::Back => buttons::BACK,
        MouseButton::Forward => buttons::FORWARD,
        MouseButton::Other(_) => 0,
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.graphics.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("OmegaDesk")
            .with_inner_size(LogicalSize::new(1280.0, 720.0));
        let created = event_loop
            .create_window(attributes)
            .map_err(|e| anyhow!("janela: {e}"))
            .and_then(|window| {
                let window = Rc::new(window);
                let context =
                    Context::new(window.clone()).map_err(|e| anyhow!("superfície: {e}"))?;
                let surface = Surface::new(&context, window.clone())
                    .map_err(|e| anyhow!("superfície: {e}"))?;
                Ok(Graphics { window, surface })
            });
        match created {
            Ok(graphics) => {
                self.graphics = Some(graphics);
                self.update_title("");
            }
            Err(error) => {
                self.error = Some(error);
                event_loop.exit();
            }
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Frame => {
                if let Some(graphics) = &self.graphics {
                    graphics.window.request_redraw();
                }
            }
            UserEvent::Status => self.update_title(""),
            UserEvent::Disconnected => {
                let status = self.shared.status();
                if status.starts_with("erro") {
                    self.error = Some(anyhow!(status));
                }
                event_loop.exit();
            }
        }
    }

    #[allow(
        clippy::wildcard_enum_match_arm,
        reason = "a janela recebe dezenas de eventos que não interessam"
    )]
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => self.redraw(),
            WindowEvent::Resized(_) => {
                if let Some(graphics) = &self.graphics {
                    graphics.window.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x, position.y);
                self.send_pointer(0, 0);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let bit = button_bit(button);
                match state {
                    ElementState::Pressed => self.pressed_buttons |= bit,
                    ElementState::Released => self.pressed_buttons &= !bit,
                }
                self.send_pointer(0, 0);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                #[allow(
                    clippy::cast_possible_truncation,
                    clippy::cast_precision_loss,
                    reason = "deltas de rolagem são pequenos"
                )]
                let (x, y) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (
                        (x * WHEEL_STEP as f32) as i32,
                        (y * WHEEL_STEP as f32) as i32,
                    ),
                    MouseScrollDelta::PixelDelta(position) => {
                        (position.x as i32, position.y as i32)
                    }
                };
                self.send_pointer(x, y);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return;
                };
                let Some(hid_usage) = keycode_to_hid(code) else {
                    return;
                };
                let keysym = match &event.logical_key {
                    Key::Character(text) => text.chars().next().map_or(0, u32::from),
                    _ => 0,
                };
                self.send(Payload::KeyEvent(KeyEvent {
                    hid_usage,
                    keysym,
                    pressed: event.state == ElementState::Pressed,
                    modifiers: 0,
                }));
            }
            // Solta os botões ao perder o foco para não deixar nada preso.
            WindowEvent::Focused(false) if self.pressed_buttons != 0 => {
                self.pressed_buttons = 0;
                self.send_pointer(0, 0);
            }
            _ => {}
        }
    }
}
