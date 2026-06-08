use anyhow::{anyhow, Result};

/// Verify the current OS user via platform-native authentication (biometric or password).
pub fn verify_os_user(prompt: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    return verify_macos(prompt);
    #[cfg(target_os = "windows")]
    return verify_windows(prompt);
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = prompt;
        Err(anyhow!(
            "OS authentication is not available on this platform"
        ))
    }
}

#[cfg(target_os = "macos")]
fn verify_macos(prompt: &str) -> Result<()> {
    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::runtime::Bool;
    use objc2_foundation::NSString;
    use objc2_local_authentication::{LAContext, LAPolicy};
    use std::sync::mpsc;
    use std::time::Duration;

    let context: Retained<LAContext> = unsafe { LAContext::new() };
    let policy = LAPolicy::DeviceOwnerAuthentication;
    if unsafe { context.canEvaluatePolicy_error(policy) }.is_err() {
        return Err(anyhow!("device authentication is not available on this Mac"));
    }

    let (tx, rx) = mpsc::channel();
    let prompt_ns = NSString::from_str(prompt);
    let block = RcBlock::new(move |success: Bool, _error: *mut objc2_foundation::NSError| {
        let _ = tx.send(success == Bool::YES);
    });

    unsafe {
        context.evaluatePolicy_localizedReason_reply(policy, &prompt_ns, &block);
    }

    match rx.recv_timeout(Duration::from_secs(120)) {
        Ok(true) => Ok(()),
        Ok(false) => Err(anyhow!("authentication was cancelled or failed")),
        Err(_) => Err(anyhow!("authentication timed out")),
    }
}

#[cfg(target_os = "windows")]
fn verify_windows(prompt: &str) -> Result<()> {
    use windows::core::HSTRING;
    use windows::Security::Credentials::UI::UserConsentVerifier;

    let message = HSTRING::from(prompt);
    let result = UserConsentVerifier::RequestVerificationAsync(&message)
        .map_err(|e| anyhow!("Windows Hello request failed: {e}"))?
        .join()
        .map_err(|e| anyhow!("Windows Hello verification failed: {e}"))?;

    use windows::Security::Credentials::UI::UserConsentVerificationResult;
    match result {
        UserConsentVerificationResult::Verified => Ok(()),
        UserConsentVerificationResult::Canceled => Err(anyhow!("authentication was cancelled")),
        _ => Err(anyhow!("authentication failed")),
    }
}
