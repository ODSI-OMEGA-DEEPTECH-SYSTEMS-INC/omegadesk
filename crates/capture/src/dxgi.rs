//! Backend DXGI Desktop Duplication (Windows 8 ou superior).
//!
//! Fluxo, no estilo do Sunshine (`src/platform/windows/display_base.cpp` e
//! `display_ram.cpp`): `IDXGIOutput1::DuplicateOutput` → `AcquireNextFrame` →
//! cópia para uma textura de staging → `Map` → BGRA na CPU, linha a linha
//! respeitando o `RowPitch`.

#![allow(unsafe_code, reason = "chamadas COM do Direct3D/DXGI")]

use std::time::{Duration, Instant};

use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ,
    D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_MODE_ROTATION_IDENTITY, DXGI_MODE_ROTATION_UNSPECIFIED,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ERROR_ACCESS_DENIED, DXGI_ERROR_ACCESS_LOST,
    DXGI_ERROR_DEVICE_REMOVED, DXGI_ERROR_INVALID_CALL, DXGI_ERROR_NOT_CURRENTLY_AVAILABLE,
    DXGI_ERROR_NOT_FOUND, DXGI_ERROR_SESSION_DISCONNECTED, DXGI_ERROR_WAIT_TIMEOUT,
    DXGI_OUTDUPL_FRAME_INFO, IDXGIDevice1, IDXGIFactory1, IDXGIOutput1, IDXGIOutputDuplication,
    IDXGIResource,
};
use windows::core::Interface;

use crate::{CaptureError, Capturer, DisplayBounds, Frame};

/// Prioridade de GPU máxima aceita sem privilégio especial (Sunshine usa 7).
const GPU_THREAD_PRIORITY: i32 = 7;

pub(crate) struct DxgiCapturer {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    duplication: IDXGIOutputDuplication,
    staging: Option<ID3D11Texture2D>,
    /// O primeiro quadro depois de abrir é sempre a imagem completa da tela.
    first_frame: bool,
    width: u32,
    height: u32,
    bounds: DisplayBounds,
}

// Os objetos COM ficam presos a esta struct e só são usados pela thread que
// chama `next_frame`; o dispositivo D3D11 é criado sem a flag single-threaded.
unsafe impl Send for DxgiCapturer {}

/// Erros que pedem para reabrir a duplicação: troca de modo, tela de login ou
/// UAC (desktop seguro), sessão desconectada, GPU reiniciada. Mesmos casos que
/// o Sunshine (`display_base.cpp`) e o `RustDesk` (`scrap/src/dxgi/mod.rs`) tratam.
fn os_error(error: &windows::core::Error) -> CaptureError {
    let code = error.code();
    if code == DXGI_ERROR_ACCESS_LOST
        || code == DXGI_ERROR_ACCESS_DENIED
        || code == DXGI_ERROR_SESSION_DISCONNECTED
        || code == DXGI_ERROR_NOT_CURRENTLY_AVAILABLE
        || code == DXGI_ERROR_DEVICE_REMOVED
    {
        CaptureError::AccessLost
    } else {
        CaptureError::Os(error.to_string())
    }
}

impl DxgiCapturer {
    pub(crate) fn open(index: u32) -> Result<Self, CaptureError> {
        unsafe {
            let factory: IDXGIFactory1 = CreateDXGIFactory1().map_err(|e| os_error(&e))?;

            // Procura o monitor `index` entre as saídas ligadas à área de
            // trabalho, em todos os adaptadores (Sunshine e CrossDesk filtram
            // `AttachedToDesktop` do mesmo jeito).
            let mut remaining = index;
            let mut adapter_index = 0;
            let (adapter, output, description) = loop {
                let adapter = match factory.EnumAdapters1(adapter_index) {
                    Ok(adapter) => adapter,
                    Err(e) if e.code() == DXGI_ERROR_NOT_FOUND => {
                        return Err(CaptureError::DisplayNotFound(index));
                    }
                    Err(e) => return Err(os_error(&e)),
                };
                let mut output_index = 0;
                let found = loop {
                    let output = match adapter.EnumOutputs(output_index) {
                        Ok(output) => output,
                        Err(e) if e.code() == DXGI_ERROR_NOT_FOUND => break None,
                        Err(e) => return Err(os_error(&e)),
                    };
                    output_index += 1;
                    let description = output.GetDesc().map_err(|e| os_error(&e))?;
                    if !description.AttachedToDesktop.as_bool() {
                        continue;
                    }
                    if remaining == 0 {
                        break Some((output, description));
                    }
                    remaining -= 1;
                };
                if let Some((output, description)) = found {
                    break (adapter, output, description);
                }
                adapter_index += 1;
            };

            let mut device = None;
            let mut context = None;
            D3D11CreateDevice(
                &adapter,
                D3D_DRIVER_TYPE_UNKNOWN,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&raw mut device),
                None,
                Some(&raw mut context),
            )
            .map_err(|e| os_error(&e))?;
            let device: ID3D11Device =
                device.ok_or_else(|| CaptureError::Os("D3D11 sem dispositivo".into()))?;
            let context: ID3D11DeviceContext =
                context.ok_or_else(|| CaptureError::Os("D3D11 sem contexto".into()))?;

            // Sem fila de quadros na GPU e com prioridade alta, como o Sunshine:
            // sob carga de jogos, a cópia e o Map travariam sem isso.
            if let Ok(dxgi_device) = device.cast::<IDXGIDevice1>() {
                let _ = dxgi_device.SetMaximumFrameLatency(1);
                let _ = dxgi_device.SetGPUThreadPriority(GPU_THREAD_PRIORITY);
            }

            let output: IDXGIOutput1 = output.cast().map_err(|e| os_error(&e))?;
            let duplication = output.DuplicateOutput(&device).map_err(|e| os_error(&e))?;
            let duplication_description = duplication.GetDesc();
            let rotation = duplication_description.Rotation;
            if rotation != DXGI_MODE_ROTATION_IDENTITY && rotation != DXGI_MODE_ROTATION_UNSPECIFIED
            {
                // Fase 3: girar o quadro, como o RustDesk (`ARGBRotate`).
                return Err(CaptureError::Os(
                    "monitor girado ainda não é suportado".into(),
                ));
            }

            let rect = description.DesktopCoordinates;
            Ok(Self {
                device,
                context,
                duplication,
                staging: None,
                first_frame: true,
                width: duplication_description.ModeDesc.Width,
                height: duplication_description.ModeDesc.Height,
                bounds: DisplayBounds {
                    x: rect.left,
                    y: rect.top,
                    width: duplication_description.ModeDesc.Width,
                    height: duplication_description.ModeDesc.Height,
                },
            })
        }
    }

    /// Textura de staging (leitura pela CPU) no formato do quadro capturado.
    fn staging_for(
        &mut self,
        description: &D3D11_TEXTURE2D_DESC,
    ) -> Result<ID3D11Texture2D, CaptureError> {
        if let Some(staging) = &self.staging {
            return Ok(staging.clone());
        }
        let mut staging_description = *description;
        staging_description.Usage = D3D11_USAGE_STAGING;
        staging_description.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0.cast_unsigned();
        staging_description.BindFlags = 0;
        staging_description.MiscFlags = 0;

        let mut staging = None;
        unsafe {
            self.device
                .CreateTexture2D(&raw const staging_description, None, Some(&raw mut staging))
                .map_err(|e| os_error(&e))?;
        }
        let staging =
            staging.ok_or_else(|| CaptureError::Os("textura de staging não criada".into()))?;
        self.staging = Some(staging.clone());
        Ok(staging)
    }

    fn copy_to_cpu(&mut self, resource: &IDXGIResource) -> Result<Vec<u8>, CaptureError> {
        let texture: ID3D11Texture2D = resource.cast().map_err(|e| os_error(&e))?;
        let mut description = D3D11_TEXTURE2D_DESC::default();
        unsafe { texture.GetDesc(&raw mut description) };

        // A cópia linha a linha usa width/height; se a textura não bater (troca
        // de modo em andamento, HDR), ler a memória mapeada seria inválido.
        // Como o Sunshine (`display_ram.cpp`), pede para reabrir.
        if description.Width != self.width
            || description.Height != self.height
            || description.Format != DXGI_FORMAT_B8G8R8A8_UNORM
        {
            return Err(CaptureError::AccessLost);
        }

        let staging = self.staging_for(&description)?;
        unsafe {
            self.context.CopyResource(&staging, &texture);

            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.context
                .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&raw mut mapped))
                .map_err(|e| os_error(&e))?;

            let row_bytes = self.width as usize * 4;
            let pitch = mapped.RowPitch as usize;
            let mut bgra = Vec::with_capacity(row_bytes * self.height as usize);
            let base = mapped.pData.cast::<u8>().cast_const();
            for row in 0..self.height as usize {
                let line = std::slice::from_raw_parts(base.add(row * pitch), row_bytes);
                bgra.extend_from_slice(line);
            }

            self.context.Unmap(&staging, 0);
            Ok(bgra)
        }
    }
}

impl Capturer for DxgiCapturer {
    fn next_frame(&mut self, timeout: Duration) -> Result<Option<Frame>, CaptureError> {
        let timeout_ms = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX);
        let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource: Option<IDXGIResource> = None;

        let acquired = unsafe {
            self.duplication
                .AcquireNextFrame(timeout_ms, &raw mut info, &raw mut resource)
        };
        match acquired {
            Ok(()) => {}
            Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => return Ok(None),
            Err(e) => return Err(os_error(&e)),
        }
        let captured_at = Instant::now();

        // Só o cursor mudou: a imagem da tela é a mesma. O primeiro quadro
        // é copiado mesmo assim, para o cliente ter uma imagem inicial.
        let result = if info.LastPresentTime == 0 && !self.first_frame {
            Ok(None)
        } else {
            match &resource {
                Some(resource) => self.copy_to_cpu(resource).map(|bgra| {
                    self.first_frame = false;
                    Some(Frame {
                        width: self.width,
                        height: self.height,
                        bgra,
                        captured_at,
                    })
                }),
                None => Ok(None),
            }
        };

        match unsafe { self.duplication.ReleaseFrame() } {
            // INVALID_CALL: o quadro já foi liberado (Sunshine ignora também).
            Err(e) if e.code() != DXGI_ERROR_INVALID_CALL => return Err(os_error(&e)),
            Ok(()) | Err(_) => {}
        }
        result
    }

    fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn bounds(&self) -> DisplayBounds {
        self.bounds
    }
}
