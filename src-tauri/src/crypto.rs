use base64::Engine;

use crate::error::{AppError, AppResult};

#[cfg(windows)]
pub fn protect_password(password: &str) -> AppResult<String> {
    use std::{ptr, slice};
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let input = CRYPT_INTEGER_BLOB {
        cbData: password.as_bytes().len() as u32,
        pbData: password.as_bytes().as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB { cbData: 0, pbData: ptr::null_mut() };

    let ok = unsafe {
        CryptProtectData(
            &input,
            ptr::null(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };

    if ok == 0 {
        return Err(AppError::Crypto("Windows DPAPI encryption failed".to_string()));
    }

    let bytes = unsafe { slice::from_raw_parts(output.pbData, output.cbData as usize) };
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    unsafe {
        LocalFree(output.pbData as _);
    }
    Ok(format!("encrypted:{encoded}"))
}

#[cfg(windows)]
pub fn reveal_password(value: &str) -> AppResult<String> {
    use std::{ptr, slice};
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let Some(encoded) = value.strip_prefix("encrypted:") else {
        return Ok(value.to_string());
    };
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|error| AppError::Crypto(error.to_string()))?;
    let input = CRYPT_INTEGER_BLOB {
        cbData: decoded.len() as u32,
        pbData: decoded.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB { cbData: 0, pbData: ptr::null_mut() };

    let ok = unsafe {
        CryptUnprotectData(
            &input,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };

    if ok == 0 {
        return Err(AppError::Crypto("Windows DPAPI decryption failed".to_string()));
    }

    let bytes = unsafe { slice::from_raw_parts(output.pbData, output.cbData as usize) };
    let password = String::from_utf8(bytes.to_vec()).map_err(|error| AppError::Crypto(error.to_string()))?;
    unsafe {
        LocalFree(output.pbData as _);
    }
    Ok(password)
}

#[cfg(not(windows))]
pub fn protect_password(password: &str) -> AppResult<String> {
    Ok(format!(
        "plain:{}",
        base64::engine::general_purpose::STANDARD.encode(password.as_bytes())
    ))
}

#[cfg(not(windows))]
pub fn reveal_password(value: &str) -> AppResult<String> {
    let Some(encoded) = value.strip_prefix("plain:") else {
        return Ok(value.to_string());
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|error| AppError::Crypto(error.to_string()))?;
    String::from_utf8(bytes).map_err(|error| AppError::Crypto(error.to_string()))
}
