//! Ajustes do sistema operacional para o agente rodar com baixa latência.

/// Pede ao sistema para não rebaixar o agente.
///
/// No Windows 11, processos sem janela em primeiro plano entram em
/// "power throttling" (`EcoQoS`) e vão para os núcleos econômicos. Num Intel
/// Core Ultra 5 135U isso deixou a codificação 17 vezes mais lenta (de ~40 ms
/// para ~680 ms por quadro 1080p). Nenhuma das referências desliga o `EcoQoS`;
/// o Sunshine só sobe a classe de prioridade (`HIGH_PRIORITY_CLASS`), o que
/// também fazemos.
pub(crate) fn prefer_performance() {
    #[cfg(windows)]
    windows_impl::prefer_performance();
}

/// Sobe a prioridade da thread atual (captura e codificação), como o Sunshine
/// faz na thread de vídeo (`src/platform/windows/misc.cpp`).
pub(crate) fn boost_current_thread() {
    #[cfg(windows)]
    windows_impl::boost_current_thread();
}

#[cfg(windows)]
mod windows_impl {
    #![allow(unsafe_code, reason = "APIs Win32 de processo")]

    use std::ffi::c_void;

    use tracing::{info, warn};
    use windows::Win32::System::Threading::{
        GetCurrentProcess, GetCurrentThread, HIGH_PRIORITY_CLASS,
        PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION, PROCESS_POWER_THROTTLING_STATE,
        ProcessPowerThrottling, SetPriorityClass, SetProcessInformation, SetThreadPriority,
        THREAD_PRIORITY_HIGHEST,
    };

    pub(super) fn prefer_performance() {
        let state = PROCESS_POWER_THROTTLING_STATE {
            Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
            // Controla velocidade de execução e resolução de timer, pedindo
            // "sem throttling" (0) para as duas.
            ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED
                | PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION,
            StateMask: 0,
        };
        let size = u32::try_from(size_of::<PROCESS_POWER_THROTTLING_STATE>()).unwrap_or(0);
        unsafe {
            let process = GetCurrentProcess();
            if let Err(error) = SetProcessInformation(
                process,
                ProcessPowerThrottling,
                (&raw const state).cast::<c_void>(),
                size,
            ) {
                warn!("não foi possível desligar o power throttling: {error}");
            }
            match SetPriorityClass(process, HIGH_PRIORITY_CLASS) {
                Ok(()) => info!("agente em prioridade alta e sem power throttling"),
                Err(error) => warn!("não foi possível subir a prioridade: {error}"),
            }
        }
    }

    pub(super) fn boost_current_thread() {
        unsafe {
            if let Err(error) = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_HIGHEST) {
                warn!("não foi possível subir a prioridade da thread: {error}");
            }
        }
    }
}
