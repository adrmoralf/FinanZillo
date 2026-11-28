//! Archivo de datos cifrado.
//!
//! Toda la base de datos SQLite vive en memoria. Para guardar se serializa
//! entera, se cifra con XChaCha20-Poly1305 y se escribe de forma atómica en un
//! único archivo `.fzl`. La clave se deriva de la contraseña con Argon2id una
//! sola vez al abrir; cada guardado usa un nonce aleatorio nuevo. En disco no
//! queda nunca una copia sin cifrar.
//!
//! Formato (todo little-endian):
//!   "FZL1" | versión u8 | m_cost u32 | t_cost u32 | p_cost u32 | sal [16] | nonce [24] | cifrado...
//! La cabecera completa va como datos asociados (AAD): cualquier cambio en ella
//! invalida el archivo.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use zeroize::{Zeroize, Zeroizing};

const MAGIC: &[u8; 4] = b"FZL1";
const FORMAT_VERSION: u8 = 1;
const HEADER_LEN: usize = 4 + 1 + 12 + 16 + 24;
// Argon2id: 64 MiB, 3 pasadas, 1 hilo (≈0,3–0,6 s en un PC normal).
const M_COST: u32 = 64 * 1024;
const T_COST: u32 = 3;
const P_COST: u32 = 1;

#[derive(Debug)]
pub enum StoreError {
    WrongPassword,
    Corrupt(String),
    Io(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::WrongPassword => write!(f, "Contraseña incorrecta"),
            StoreError::Corrupt(m) => write!(f, "El archivo de datos está dañado: {m}"),
            StoreError::Io(m) => write!(f, "No se pudo leer o escribir el archivo: {m}"),
        }
    }
}

impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        StoreError::Io(e.to_string())
    }
}

/// Clave derivada + parámetros, listos para volver a cifrar sin pedir la contraseña.
pub struct Sealer {
    key: Zeroizing<[u8; 32]>,
    salt: [u8; 16],
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
}

fn random_bytes<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    getrandom::fill(&mut b).expect("generador aleatorio del sistema");
    b
}

pub(crate) fn random<const N: usize>() -> [u8; N] {
    random_bytes::<N>()
}

fn derive_key(password: &str, salt: &[u8; 16], m: u32, t: u32, p: u32) -> Result<Zeroizing<[u8; 32]>, StoreError> {
    derive_raw(password.as_bytes(), salt, m, t, p)
}

/// Argon2id sobre bytes cualesquiera (también la firma de Windows Hello).
pub(crate) fn derive_raw(input: &[u8], salt: &[u8], m: u32, t: u32, p: u32) -> Result<Zeroizing<[u8; 32]>, StoreError> {
    let params = Params::new(m, t, p, Some(32)).map_err(|e| StoreError::Corrupt(e.to_string()))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = Zeroizing::new([0u8; 32]);
    argon
        .hash_password_into(input, salt, key.as_mut())
        .map_err(|e| StoreError::Corrupt(e.to_string()))?;
    Ok(key)
}

impl Sealer {
    /// Clave nueva con sal nueva (archivo nuevo o cambio de contraseña).
    pub fn new(password: &str) -> Result<Self, StoreError> {
        let salt = random_bytes::<16>();
        let key = derive_key(password, &salt, M_COST, T_COST, P_COST)?;
        Ok(Sealer { key, salt, m_cost: M_COST, t_cost: T_COST, p_cost: P_COST })
    }

    fn header(&self, nonce: &[u8; 24]) -> Vec<u8> {
        let mut h = Vec::with_capacity(HEADER_LEN);
        h.extend_from_slice(MAGIC);
        h.push(FORMAT_VERSION);
        h.extend_from_slice(&self.m_cost.to_le_bytes());
        h.extend_from_slice(&self.t_cost.to_le_bytes());
        h.extend_from_slice(&self.p_cost.to_le_bytes());
        h.extend_from_slice(&self.salt);
        h.extend_from_slice(nonce);
        h
    }

    pub fn seal(&self, plaintext: &[u8]) -> Result<Vec<u8>, StoreError> {
        let nonce = random_bytes::<24>();
        let header = self.header(&nonce);
        let cipher = XChaCha20Poly1305::new(self.key.as_ref().into());
        let ct = cipher
            .encrypt(XNonce::from_slice(&nonce), Payload { msg: plaintext, aad: &header })
            .map_err(|_| StoreError::Corrupt("fallo al cifrar".into()))?;
        let mut out = header;
        out.extend_from_slice(&ct);
        Ok(out)
    }

    /// Clave y sal actuales (para envolverlas con Windows Hello).
    pub fn key_material(&self) -> (Zeroizing<[u8; 32]>, [u8; 16]) {
        (Zeroizing::new(*self.key), self.salt)
    }

    /// Descifra un archivo con la contraseña. Devuelve el sellador (para
    /// guardados posteriores) y la base de datos en claro.
    pub fn open(bytes: &[u8], password: &str) -> Result<(Self, Zeroizing<Vec<u8>>), StoreError> {
        let h = parse_header(bytes)?;
        let key = derive_key(password, &h.salt, h.m, h.t, h.p)?;
        Self::decrypt(bytes, h, key)
    }

    /// Descifra con la clave ya derivada (desbloqueo con Windows Hello).
    pub fn open_with_key(bytes: &[u8], key: Zeroizing<[u8; 32]>) -> Result<(Self, Zeroizing<Vec<u8>>), StoreError> {
        let h = parse_header(bytes)?;
        Self::decrypt(bytes, h, key)
    }

    fn decrypt(bytes: &[u8], h: Header, key: Zeroizing<[u8; 32]>) -> Result<(Self, Zeroizing<Vec<u8>>), StoreError> {
        let cipher = XChaCha20Poly1305::new(key.as_ref().into());
        let pt = cipher
            .decrypt(XNonce::from_slice(&h.nonce), Payload { msg: &bytes[HEADER_LEN..], aad: &bytes[..HEADER_LEN] })
            .map_err(|_| StoreError::WrongPassword)?;
        Ok((Sealer { key, salt: h.salt, m_cost: h.m, t_cost: h.t, p_cost: h.p }, Zeroizing::new(pt)))
    }
}

struct Header {
    m: u32,
    t: u32,
    p: u32,
    salt: [u8; 16],
    nonce: [u8; 24],
}

/// Lee y valida la cabecera del archivo.
fn parse_header(bytes: &[u8]) -> Result<Header, StoreError> {
    if bytes.len() < HEADER_LEN + 16 || &bytes[0..4] != MAGIC {
        return Err(StoreError::Corrupt("no es un archivo de FinanZillo".into()));
    }
    if bytes[4] != FORMAT_VERSION {
        return Err(StoreError::Corrupt(format!("versión de formato {} no soportada", bytes[4])));
    }
    let rd = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
    let (m, t, p) = (rd(5), rd(9), rd(13));
    // Límites razonables: un archivo manipulado no puede pedir 100 GB de RAM.
    if !(8 * 1024..=1024 * 1024).contains(&m) || !(1..=20).contains(&t) || !(1..=8).contains(&p) {
        return Err(StoreError::Corrupt("parámetros de cifrado fuera de rango".into()));
    }
    Ok(Header { m, t, p, salt: bytes[17..33].try_into().unwrap(), nonce: bytes[33..57].try_into().unwrap() })
}

/// Sal del archivo cifrado (sin descifrarlo).
pub fn file_salt(bytes: &[u8]) -> Option<[u8; 16]> {
    parse_header(bytes).ok().map(|h| h.salt)
}

/// Carpeta de datos: %APPDATA%\FinanZillo (o FINANZILLO_DATA_DIR para pruebas).
pub fn data_dir() -> PathBuf {
    if let Ok(d) = std::env::var("FINANZILLO_DATA_DIR") {
        if !d.trim().is_empty() {
            return PathBuf::from(d);
        }
    }
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("FinanZillo")
}

pub fn data_file() -> PathBuf {
    data_dir().join("finanzillo.fzl")
}

pub fn backups_dir() -> PathBuf {
    data_dir().join("copias")
}

/// Escritura atómica: archivo temporal en la misma carpeta + renombrado.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("fzl.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Copia diaria del archivo cifrado en `copias/` (y en una carpeta externa
/// opcional, p. ej. OneDrive). Conserva las últimas `keep` copias.
pub fn daily_backup(sealed: &[u8], external: Option<&Path>, keep: usize) -> Result<Option<PathBuf>, StoreError> {
    let name = format!("finanzillo-{}.fzl", crate::util::today_str());
    let mut written = None;
    let mut dirs_to_use = vec![backups_dir()];
    if let Some(ext) = external {
        dirs_to_use.push(ext.to_path_buf());
    }
    for dir in dirs_to_use {
        let target = dir.join(&name);
        if target.exists() {
            continue;
        }
        if fs::create_dir_all(&dir).is_err() {
            continue;
        }
        write_atomic(&target, sealed)?;
        written.get_or_insert(target);
        prune(&dir, keep);
    }
    Ok(written)
}

fn prune(dir: &Path, keep: usize) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut files: Vec<PathBuf> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("finanzillo-") && n.ends_with(".fzl"))
        })
        .collect();
    files.sort();
    if files.len() > keep {
        for f in &files[..files.len() - keep] {
            let _ = fs::remove_file(f);
        }
    }
}

/// Borra de memoria un búfer con datos en claro.
pub fn wipe(mut v: Vec<u8>) {
    v.zeroize();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lo que usa el PIN de Windows: abrir con la clave ya derivada.
    #[test]
    fn abre_con_la_clave() {
        let s = Sealer::new("contraseña de prueba").unwrap();
        let sealed = s.seal(b"datos").unwrap();
        let (key, salt) = s.key_material();
        assert_eq!(file_salt(&sealed), Some(salt));
        let (_, plain) = Sealer::open_with_key(&sealed, key).unwrap();
        assert_eq!(plain.as_slice(), b"datos");
        assert!(Sealer::open_with_key(&sealed, Zeroizing::new([7u8; 32])).is_err());
    }

    #[test]
    fn ida_y_vuelta() {
        let s = Sealer::new("clave de prueba").unwrap();
        let sealed = s.seal(b"hola finanzas").unwrap();
        assert!(!sealed.windows(5).any(|w| w == b"finan"));
        let (s2, pt) = Sealer::open(&sealed, "clave de prueba").unwrap();
        assert_eq!(&pt[..], b"hola finanzas");
        // El sellador recuperado vuelve a cifrar y se puede abrir otra vez.
        let again = s2.seal(b"otra vez").unwrap();
        assert_eq!(&Sealer::open(&again, "clave de prueba").unwrap().1[..], b"otra vez");
    }

    #[test]
    fn contrasena_mala_y_manipulacion() {
        let s = Sealer::new("buena").unwrap();
        let mut sealed = s.seal(b"datos").unwrap();
        assert!(matches!(Sealer::open(&sealed, "mala"), Err(StoreError::WrongPassword)));
        let last = sealed.len() - 1;
        sealed[last] ^= 1;
        assert!(Sealer::open(&sealed, "buena").is_err());
    }
}
