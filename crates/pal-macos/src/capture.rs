//! macOS CGEventTap-based capture (skeleton).
//!
//! v0.2 ships a working scaffold: `start` checks `AXIsProcessTrusted`
//! and surfaces a friendly error if the borderless binary hasn't been
//! granted Accessibility + Input Monitoring permissions. The actual
//! `CGEventTapCreate` + `CFRunLoopRun` plumbing is deliberately
//! deferred — running on macOS without Accessibility consent is the
//! single most common reason capture would silently fail, and v0.2's
//! Hub-only-active runtime lets the doctor command guide users
//! through fixing it before turning the tap on. The full event tap
//! lands in v0.3.

use async_trait::async_trait;
use borderless_pal::{CaptureMode, EventSink, InputCapture, PalError, PalResult};
use tracing::{debug, warn};

/// CGEventTap capture handle.
pub struct MacosCapture {
    mode: CaptureMode,
}

impl MacosCapture {
    /// New capture (idle until `start`).
    pub fn new() -> Self {
        Self {
            mode: CaptureMode::Off,
        }
    }
}

impl Default for MacosCapture {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns `true` if the borderless binary has been granted
/// Accessibility permissions.
pub fn is_process_trusted() -> bool {
    use core_foundation::base::TCFType;
    use core_foundation::dictionary::{CFDictionary, CFMutableDictionary};
    use core_foundation::string::CFString;

    extern "C" {
        fn AXIsProcessTrustedWithOptions(options: *const std::ffi::c_void) -> bool;
    }

    let key = CFString::from_static_string("AXTrustedCheckOptionPrompt");
    let mut opts = CFMutableDictionary::<CFString, core_foundation::boolean::CFBoolean>::new();
    // Don't prompt; we surface this through `borderless doctor`.
    opts.add(
        &key.as_concrete_TypeRef().into(),
        &core_foundation::boolean::CFBoolean::false_value(),
    );
    let dict: CFDictionary = opts.to_immutable();
    unsafe { AXIsProcessTrustedWithOptions(dict.as_concrete_TypeRef() as *const _) }
}

#[async_trait]
impl InputCapture for MacosCapture {
    async fn start(&mut self, _sink: EventSink) -> PalResult<()> {
        if !is_process_trusted() {
            return Err(PalError::PermissionRequired(
                "Accessibility permission is required for input capture on macOS. \
                 Open System Settings → Privacy & Security → Accessibility and \
                 enable `borderless`."
                    .into(),
            ));
        }
        warn!("MacosCapture::start scaffold: full CGEventTap loop arrives in v0.3");
        Ok(())
    }

    async fn stop(&mut self) -> PalResult<()> {
        Ok(())
    }

    async fn set_mode(&mut self, mode: CaptureMode) -> PalResult<()> {
        debug!(?mode, "MacosCapture::set_mode");
        self.mode = mode;
        Ok(())
    }
}
