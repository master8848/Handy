/// Whether this is the x64 Windows build running under emulation on Windows ARM64.
/// Ported from Handy's utils.rs (same logic, no Tauri).
pub fn is_windows_x64_emulated_on_arm64() -> bool {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        use windows::Win32::System::SystemInformation::{
            GetNativeSystemInfo, PROCESSOR_ARCHITECTURE_ARM64, SYSTEM_INFO,
        };
        let mut info = SYSTEM_INFO::default();
        unsafe { GetNativeSystemInfo(&mut info) };
        info.Anonymous.Anonymous.wProcessorArchitecture == PROCESSOR_ARCHITECTURE_ARM64
    }
    #[cfg(not(all(target_os = "windows", target_arch = "x86_64")))]
    {
        false
    }
}
