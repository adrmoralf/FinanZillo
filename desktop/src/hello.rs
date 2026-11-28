//! Desbloqueo con Windows Hello (el PIN del ordenador, huella o cara).
//!
//! Al activarlo se crea con Windows Hello una clave «FinanZillo» que vive en
//! el TPM del equipo y solo firma tras pedir el PIN. Se firma un reto
//! aleatorio; la firma (RSA PKCS#1 v1.5, determinista) pasa por Argon2id y da
//! una clave que cifra la clave del archivo `.fzl`. En `hello.fzk` se guarda:
//!
//!   "FZH1" | sal del .fzl [16] | reto [32] | nonce [24] | clave cifrada [32+16]
//!
//! La cabecera va como datos asociados. Sin el PIN de este equipo la firma no
//! se puede reproducir, así que el archivo solo no sirve de nada. La
//! contraseña sigue funcionando siempre (y es la única vía en otro equipo).

use std::path::PathBuf;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use zeroize::Zeroizing;

use crate::store::{self, Sealer};

const MAGIC: &[u8; 4] = b"FZH1";
const HEAD: usize = 4 + 16 + 32 + 24;
const NAME: &str = "FinanZillo";

pub fn file() -> PathBuf {
    store::data_dir().join("hello.fzk")
}

pub fn enabled() -> bool {
    file().exists()
}

/// ¿Tiene este equipo Windows Hello configurado (PIN, huella o cara)?
pub fn supported() -> bool {
    imp::supported()
}

fn wrap_key(signature: &[u8], challenge: &[u8; 32]) -> Result<Zeroizing<[u8; 32]>, String> {
    // La firma ya es secreta y de alta entropía: Argon2id ligero basta.
    store::derive_raw(signature, &challenge[..16], 8 * 1024, 1, 1).map_err(|e| e.to_string())
}

/// Activa el desbloqueo con PIN para la clave actual. Pide el PIN.
pub fn enable(sealer: (Zeroizing<[u8; 32]>, [u8; 16])) -> Result<(), String> {
    let (key, salt) = sealer;
    let challenge = store::random::<32>();
    let sig = Zeroizing::new(imp::sign(&challenge, true)?);
    let wk = wrap_key(&sig, &challenge)?;
    let nonce = store::random::<24>();
    let mut head = Vec::with_capacity(HEAD);
    head.extend_from_slice(MAGIC);
    head.extend_from_slice(&salt);
    head.extend_from_slice(&challenge);
    head.extend_from_slice(&nonce);
    let ct = XChaCha20Poly1305::new(wk.as_ref().into())
        .encrypt(XNonce::from_slice(&nonce), Payload { msg: key.as_ref(), aad: &head })
        .map_err(|_| "fallo al cifrar".to_string())?;
    let mut out = head;
    out.extend_from_slice(&ct);
    store::write_atomic(&file(), &out).map_err(|e| e.to_string())
}

/// Desbloquea el archivo de datos con el PIN. Pide el PIN.
pub fn unlock(fzl: &[u8]) -> Result<(Sealer, Zeroizing<Vec<u8>>), String> {
    let raw = std::fs::read(file()).map_err(|_| "El desbloqueo con PIN no está activado".to_string())?;
    if raw.len() != HEAD + 48 || &raw[..4] != MAGIC {
        return Err("Los datos del PIN están dañados: entra con la contraseña y vuelve a activarlo en Ajustes".into());
    }
    let salt: [u8; 16] = raw[4..20].try_into().unwrap();
    if store::file_salt(fzl) != Some(salt) {
        return Err("La contraseña cambió desde que activaste el PIN: entra con la contraseña y vuelve a activarlo en Ajustes".into());
    }
    let challenge: [u8; 32] = raw[20..52].try_into().unwrap();
    let nonce: [u8; 24] = raw[52..76].try_into().unwrap();
    let sig = Zeroizing::new(imp::sign(&challenge, false)?);
    let wk = wrap_key(&sig, &challenge)?;
    let key = XChaCha20Poly1305::new(wk.as_ref().into())
        .decrypt(XNonce::from_slice(&nonce), Payload { msg: &raw[HEAD..], aad: &raw[..HEAD] })
        .map_err(|_| "Windows Hello no devolvió la clave esperada: entra con la contraseña y vuelve a activar el PIN".to_string())?;
    let key: Zeroizing<[u8; 32]> = Zeroizing::new(key.as_slice().try_into().map_err(|_| "clave no válida".to_string())?);
    Sealer::open_with_key(fzl, key).map_err(|e| e.to_string())
}

/// Desactiva el PIN: borra `hello.fzk` y la clave de Windows Hello.
pub fn disable() {
    let _ = std::fs::remove_file(file());
    imp::delete();
}

#[cfg(windows)]
mod imp {
    use windows::Security::Credentials::{KeyCredentialCreationOption, KeyCredentialManager, KeyCredentialStatus};
    use windows::Security::Cryptography::CryptographicBuffer;
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, SetForegroundWindow};
    use windows::core::{Array, HSTRING, w};

    pub fn supported() -> bool {
        KeyCredentialManager::IsSupportedAsync().and_then(|op| op.join()).unwrap_or(false)
    }

    /// La ventana del PIN puede abrirse detrás de la app: se trae al frente
    /// durante unos segundos mientras se espera la respuesta.
    fn bring_prompt_forward() {
        std::thread::spawn(|| {
            for _ in 0..40 {
                std::thread::sleep(std::time::Duration::from_millis(100));
                unsafe {
                    if let Ok(h) = FindWindowW(w!("Credential Dialog Xaml Host"), None) {
                        if !h.is_invalid() {
                            let _ = SetForegroundWindow(h);
                            return;
                        }
                    }
                }
            }
        });
    }

    fn status_err(s: KeyCredentialStatus) -> String {
        match s {
            KeyCredentialStatus::UserCanceled => "Has cancelado el PIN de Windows. Escribe la contraseña o vuelve a intentarlo.".into(),
            KeyCredentialStatus::NotFound => "La clave de Windows Hello de FinanZillo ya no existe: entra con la contraseña y vuelve a activar el PIN".into(),
            KeyCredentialStatus::UserPrefersPassword => "Has elegido usar la contraseña".into(),
            KeyCredentialStatus::SecurityDeviceLocked => "El dispositivo de seguridad está bloqueado por demasiados intentos".into(),
            _ => "Windows Hello no está disponible ahora".into(),
        }
    }

    pub fn sign(challenge: &[u8], create: bool) -> Result<Vec<u8>, String> {
        let e = |e: windows::core::Error| format!("Windows Hello: {}", e.message());
        let name = HSTRING::from(super::NAME);
        bring_prompt_forward();
        let res = if create {
            KeyCredentialManager::RequestCreateAsync(&name, KeyCredentialCreationOption::ReplaceExisting)
        } else {
            KeyCredentialManager::OpenAsync(&name)
        }
        .and_then(|op| op.join())
        .map_err(e)?;
        let st = res.Status().map_err(e)?;
        if st != KeyCredentialStatus::Success {
            return Err(status_err(st));
        }
        let cred = res.Credential().map_err(e)?;
        let buf = CryptographicBuffer::CreateFromByteArray(challenge).map_err(e)?;
        bring_prompt_forward();
        let r = cred.RequestSignAsync(&buf).and_then(|op| op.join()).map_err(e)?;
        let st = r.Status().map_err(e)?;
        if st != KeyCredentialStatus::Success {
            return Err(status_err(st));
        }
        let sig = r.Result().map_err(e)?;
        let mut arr = Array::<u8>::new();
        CryptographicBuffer::CopyToByteArray(&sig, &mut arr).map_err(e)?;
        Ok(arr.to_vec())
    }

    pub fn delete() {
        let _ = KeyCredentialManager::DeleteAsync(&HSTRING::from(super::NAME)).and_then(|op| op.join());
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn supported() -> bool {
        false
    }
    pub fn sign(_: &[u8], _: bool) -> Result<Vec<u8>, String> {
        Err("Windows Hello solo existe en Windows".into())
    }
    pub fn delete() {}
}

#[cfg(all(test, windows))]
mod tests {
    /// Sin PIN: abrir una clave que no existe responde «no existe» sin
    /// mostrar nada. Comprueba que las llamadas a Windows funcionan.
    #[test]
    #[ignore = "habla con Windows Hello del equipo"]
    fn clave_inexistente() {
        use windows::Security::Credentials::{KeyCredentialManager, KeyCredentialStatus};
        let r = KeyCredentialManager::OpenAsync(&windows::core::HSTRING::from("FinanZillo-prueba-inexistente")).unwrap().join().unwrap();
        assert_eq!(r.Status().unwrap(), KeyCredentialStatus::NotFound);
        assert!(super::imp::supported());
    }
}
