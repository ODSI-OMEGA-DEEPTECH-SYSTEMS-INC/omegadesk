//! Backend DXGI Desktop Duplication (Windows 8 ou superior).
//!
//! Fluxo, no estilo do Sunshine (`src/platform/windows/display_base.cpp`):
//! `IDXGIOutput1::DuplicateOutput` → `AcquireNextFrame` → cópia para uma
//! textura de staging → `Map` → BGRA na CPU.

#![allow(unsafe_code, reason = "chamadas COM do Direct3D/DXGI")]

use std::time::{Duration, Instant};

use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ,
    D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_NOT_FOUND, DXGI_ERROR_WAIT_TIMEOUT,
    DXGI_OUTDUPL_FRAME_INFO, IDXGIFactory1, IDXGIOutput1, IDXGIOutputDuplication, IDXGIResource,
};
use windows::core::Interface;

use crate::{CaptureError, Capturer, Frame};

pub(crate) struct DxgiCapturer {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    duplication: IDXGIOutputDuplication,
    staging: Option<ID3D11Texture2D>,
    width: u32,
    height: u32,
}

// Os objetos COM ficam presos a esta struct e só são usados pela thread que
// chama `next_frame`; o dispositivo D3D11 é criado sem a flag single-threaded.
unsafe impl Send for DxgiCapturer {}

fn os_error(error: &windows::core::Error) -> CaptureError {
    if error.code() == DXGI_ERROR_ACCESS_LOST {
        CaptureError::AccessLost
    } else {
        CaptureError::Os(error.to_string())
    }
}

impl DxgiCapturer {
    pub(crate) fn open(index: u32) -> Result<Self, CaptureError> {
        unsafe {
            let factory: IDXGIFactory1 = CreateDXGIFactory1().map_err(|e| os_error(&e))?;

            // Procura o monitor `index` percorrendo as saídas de todos os adaptadores.
            let mut remaining = index;
            let mut adapter_index = 0;
            let (adapter, output) = loop {
                let adapter = match factory.EnumAdapters1(adapter_index) {
                    Ok(adapter) => adapter,
                    Err(e) if e.code() == DXGI_ERROR_NOT_FOUND => {
                        return Err(CaptureError::DisplayNotFound(index));
                    }
                    Err(e) => return Err(os_error(&e)),
                };
                let mut output_index = 0;
                let found = loop {
                    match adapter.EnumOutputs(output_index) {
                        Ok(output) if remaining == 0 => break Some(output),
                        Ok(_) => remaining -= 1,
                        Err(e) if e.code() == DXGI_ERROR_NOT_FOUND => break None,
                        Err(e) => return Err(os_error(&e)),
                    }
                    output_index += 1;
                };
                if let Some(output) = found {
                    break (adapter, output);
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

            let output: IDXGIOutput1 = output.cast().map_err(|e| os_error(&e))?;
            let duplication = output
                .DuplicateOutput(&device)
                .map_err(|e| os_error(&e))?;
            let description = duplication.GetDesc();

            Ok(Self {
                device,
                context,
                duplication,
                staging: None,
                width: description.ModeDesc.Width,
                height: description.ModeDesc.Height,
            })
        }
    }

    /// Textura de staging (leitura pela CPU) no formato do quadro capturado.
    fn staging_for(&mut self, source: &ID3D11Texture2D) -> Result<ID3D11Texture2D, CaptureError> {
        if let Some(staging) = &self.staging {
            return Ok(staging.clone());
        }
        unsafe {
            let mut description = D3D11_TEXTURE2D_DESC::default();
            source.GetDesc(&raw mut description);
            description.Usage = D3D11_USAGE_STAGING;
            description.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0.cast_unsigned();
            description.BindFlags = 0;
            description.MiscFlags = 0;

            let mut staging = None;
            self.device
                .CreateTexture2D(&raw const description, None, Some(&raw mut staging))
                .map_err(|e| os_error(&e))?;
            let staging =
                staging.ok_or_else(|| CaptureError::Os("textura de staging não criada".into()))?;
            self.staging = Some(staging.clone());
            Ok(staging)
        }
    }

    fn copy_to_cpu(&mut self, resource: &IDXGIResource) -> Result<Vec<u8>, CaptureError> {
        let texture: ID3D11Texture2D = resource.cast().map_err(|e| os_error(&e))?;
        let staging = self.staging_for(&texture)?;
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

        // Só o cursor mudou: a imagem da tela é a mesma.
        let result = if info.LastPresentTime == 0 {
            Ok(None)
        } else {
            match &resource {
                Some(resource) => self.copy_to_cpu(resource).map(|bgra| {
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

        unsafe {
            self.duplication.ReleaseFrame().map_err(|e| os_error(&e))?;
        }
        result
    }

    fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}
