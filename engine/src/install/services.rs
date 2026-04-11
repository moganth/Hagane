use anyhow::{Context, Result};
use crate::parser::schema::ServiceOperation;

pub fn apply_service_step(
    operation: &ServiceOperation,
    name: &str,
    display_name: Option<&str>,
    executable: Option<&str>,
    start_type: Option<&str>,
    _description: Option<&str>,
) -> Result<()> {
    #[cfg(windows)]
    {
        use windows::Win32::System::Services::*;
        use windows::core::PCSTR;

        let scm = unsafe {
            OpenSCManagerA(PCSTR::null(), PCSTR::null(), SC_MANAGER_ALL_ACCESS)
                .context("OpenSCManagerA failed — are you running as Administrator?")?
        };

        let result = match operation {
            ServiceOperation::Install => {
                let exe = executable.ok_or_else(|| anyhow::anyhow!("executable required for service install"))?;
                let stype = parse_start_type(start_type.unwrap_or("auto"));
                let name_cstr = std::ffi::CString::new(name)?;
                let disp_cstr = std::ffi::CString::new(display_name.unwrap_or(name))?;
                let exe_cstr = std::ffi::CString::new(exe)?;
                unsafe {
                    CreateServiceA(
                        scm,
                        PCSTR(name_cstr.as_ptr() as *const u8),
                        PCSTR(disp_cstr.as_ptr() as *const u8),
                        SERVICE_ALL_ACCESS,
                        SERVICE_WIN32_OWN_PROCESS,
                        stype,
                        SERVICE_ERROR_NORMAL,
                        PCSTR(exe_cstr.as_ptr() as *const u8),
                        PCSTR::null(), None, PCSTR::null(), PCSTR::null(), PCSTR::null(),
                    ).map(|_| ())
                    .map_err(|e| anyhow::anyhow!("CreateServiceA failed: {}", e))
                }
            }
            ServiceOperation::Start => {
                let name_cstr = std::ffi::CString::new(name)?;
                unsafe {
                    let svc = OpenServiceA(scm, PCSTR(name_cstr.as_ptr() as *const u8), SERVICE_START)
                        .context("OpenServiceA failed")?;
                    StartServiceA(svc, None)
                        .map_err(|e| anyhow::anyhow!("StartServiceA failed: {}", e))
                }
            }
            ServiceOperation::Stop => {
                let name_cstr = std::ffi::CString::new(name)?;
                unsafe {
                    let svc = OpenServiceA(scm, PCSTR(name_cstr.as_ptr() as *const u8), SERVICE_STOP)
                        .context("OpenServiceA failed")?;
                    let mut status = SERVICE_STATUS::default();
                    ControlService(svc, SERVICE_CONTROL_STOP, &mut status)
                        .map_err(|e| anyhow::anyhow!("ControlService STOP failed: {}", e))
                }
            }
            ServiceOperation::Delete => {
                let name_cstr = std::ffi::CString::new(name)?;
                unsafe {
                    let svc = OpenServiceA(scm, PCSTR(name_cstr.as_ptr() as *const u8), SERVICE_ALL_ACCESS)
                        .context("OpenServiceA failed")?;
                    DeleteService(svc)
                        .map_err(|e| anyhow::anyhow!("DeleteService failed: {}", e))
                }
            }
        };

        unsafe { CloseServiceHandle(scm).ok(); }
        result
    }
    #[cfg(not(windows))]
    {
        log::warn!("Service operations not supported on non-Windows");
        Ok(())
    }
}

#[cfg(windows)]
fn parse_start_type(s: &str) -> windows::Win32::System::Services::SERVICE_START_TYPE {
    use windows::Win32::System::Services::*;
    match s {
        "auto"     => SERVICE_AUTO_START,
        "manual"   => SERVICE_DEMAND_START,
        "disabled" => SERVICE_DISABLED,
        _          => SERVICE_AUTO_START,
    }
}