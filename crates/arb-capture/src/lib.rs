//! Versioned immutable capture bundles with integrity checks and an incomplete-write marker.
use arb_adapter_api::{Chain, StateContext};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fmt,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
pub const MAX_BUNDLE_BYTES: u64 = 64 * 1024 * 1024;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureError(pub &'static str);
impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for CaptureError {}
pub type Result<T> = std::result::Result<T, CaptureError>;
pub fn digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}
fn valid_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|s| {
        s.len() == 64
            && s.bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    })
}
fn label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
        && value != "."
        && value != ".."
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Origin {
    Synthetic,
    ManuallyConstructed,
    RecordedLive,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectRef {
    pub name: String,
    pub sha256: String,
    pub bytes: u64,
    pub content_type: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureManifest {
    pub schema_version: u32,
    pub capture_id: String,
    pub origin: Origin,
    pub network: Chain,
    /// Non-secret configured alias, never an endpoint URL or credentials.
    pub provider_alias: String,
    pub adapter_version: String,
    pub adapter_source_commit: String,
    pub build_digest: String,
    pub config_digest: String,
    pub created_at_ms: u64,
    pub raw_expires_at_ms: Option<u64>,
    pub context: StateContext,
    pub first_sequence: u64,
    pub last_sequence: u64,
    pub required_inputs: Vec<String>,
    pub missing_inputs: Vec<String>,
    pub coherent: bool,
    pub complete_for_quote: bool,
    pub objects: Vec<ObjectRef>,
}
impl CaptureManifest {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(CaptureError("unsupported capture schema"));
        }
        if !label(&self.capture_id)
            || !label(&self.provider_alias)
            || !label(&self.adapter_version)
            || self.adapter_source_commit.len() != 40
            || !self
                .adapter_source_commit
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
            || !valid_digest(&self.build_digest)
            || !valid_digest(&self.config_digest)
        {
            return Err(CaptureError("invalid capture provenance"));
        }
        if self.created_at_ms == 0
            || self
                .raw_expires_at_ms
                .is_some_and(|x| x <= self.created_at_ms)
            || self.first_sequence > self.last_sequence
        {
            return Err(CaptureError("invalid capture time or ordering"));
        }
        match (&self.network, &self.context) {
            (
                Chain::BaseMainnet,
                StateContext::Evm {
                    block_hash,
                    parent_hash,
                    finality,
                    ..
                },
            ) if valid_evm_hash(block_hash)
                && valid_evm_hash(parent_hash)
                && matches!(finality.as_str(), "finalized" | "safe") => {}
            (
                Chain::SolanaMainnet,
                StateContext::Solana {
                    genesis_hash,
                    commitment,
                    account_context,
                    ..
                },
            ) if !genesis_hash.is_empty()
                && commitment == "finalized"
                && account_context == "single-getMultipleAccounts-response" => {}
            _ => {
                return Err(CaptureError(
                    "chain/context mismatch or unsupported finality policy",
                ));
            }
        }
        let required: BTreeSet<_> = self.required_inputs.iter().collect();
        let missing: BTreeSet<_> = self.missing_inputs.iter().collect();
        if required.is_empty()
            || required.len() != self.required_inputs.len()
            || missing.len() != self.missing_inputs.len()
            || !missing.is_subset(&required)
            || (self.complete_for_quote && (!self.coherent || !missing.is_empty()))
        {
            return Err(CaptureError("invalid completeness declaration"));
        }
        if self.objects.is_empty() || self.objects.len() > 4096 {
            return Err(CaptureError("missing objects or object quota exceeded"));
        }
        let mut names = BTreeSet::new();
        let mut total = 0_u64;
        for object in &self.objects {
            if !label(&object.name)
                || object.name == "manifest.json"
                || object.name == "INCOMPLETE"
                || object.name.ends_with(".tmp")
                || !names.insert(&object.name)
                || !valid_digest(&object.sha256)
                || object.bytes == 0
                || object.content_type != "application/json"
            {
                return Err(CaptureError("invalid capture object"));
            }
            total = total
                .checked_add(object.bytes)
                .ok_or(CaptureError("capture size overflow"))?;
        }
        if total > MAX_BUNDLE_BYTES {
            return Err(CaptureError("capture byte quota exceeded"));
        }
        Ok(())
    }
    /// Recorded input provenance is necessary but never sufficient for a performance claim.
    pub fn is_recorded_market_input(&self) -> bool {
        self.origin == Origin::RecordedLive
    }
}
fn valid_evm_hash(s: &str) -> bool {
    s.len() == 66 && s.starts_with("0x") && s[2..].bytes().all(|b| b.is_ascii_hexdigit())
}
fn write_sync(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| CaptureError("capture file creation failed"))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| CaptureError("capture durable write failed"))
}
fn sync_dir(path: &Path) -> Result<()> {
    File::open(path)
        .and_then(|x| x.sync_all())
        .map_err(|_| CaptureError("capture directory sync failed"))
}

/// Allocates a new immutable directory. Any failure after allocation leaves INCOMPLETE,
/// or lacks a manifest; both conditions are unconditionally rejected by load_bundle.
/// One caller owns a bundle directory; quota applies to this bundle, not the whole disk.
pub fn write_bundle(
    path: &Path,
    mut manifest: CaptureManifest,
    payloads: Vec<(String, Vec<u8>)>,
    quota_bytes: u64,
) -> Result<String> {
    if quota_bytes == 0 || quota_bytes > MAX_BUNDLE_BYTES {
        return Err(CaptureError("invalid capture quota"));
    }
    fs::create_dir(path)
        .map_err(|_| CaptureError("capture directory exists or cannot be created"))?;
    write_sync(&path.join("INCOMPLETE"), b"capture not committed\n")?;
    sync_dir(path)?;
    manifest.objects = payloads
        .iter()
        .map(|(name, bytes)| ObjectRef {
            name: name.clone(),
            sha256: digest(bytes),
            bytes: bytes.len() as u64,
            content_type: "application/json".into(),
        })
        .collect();
    manifest.validate()?;
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|_| CaptureError("manifest encoding failed"))?;
    let total = manifest
        .objects
        .iter()
        .map(|x| x.bytes)
        .sum::<u64>()
        .checked_add(manifest_bytes.len() as u64)
        .ok_or(CaptureError("capture size overflow"))?;
    if total > quota_bytes || manifest_bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(CaptureError("capture byte quota exceeded"));
    }
    for (name, bytes) in payloads {
        // A new directory and validated flat names prevent traversal and overwrite.
        write_sync(&path.join(format!("{name}.tmp")), &bytes)?;
        fs::rename(path.join(format!("{name}.tmp")), path.join(name))
            .map_err(|_| CaptureError("capture object publication failed"))?;
    }
    write_sync(&path.join("manifest.json.tmp"), &manifest_bytes)?;
    fs::rename(path.join("manifest.json.tmp"), path.join("manifest.json"))
        .map_err(|_| CaptureError("manifest publication failed"))?;
    sync_dir(path)?;
    fs::remove_file(path.join("INCOMPLETE"))
        .map_err(|_| CaptureError("capture commit marker failed"))?;
    sync_dir(path)?;
    if let Some(parent) = path.parent().filter(|x| !x.as_os_str().is_empty()) {
        sync_dir(parent)?;
    }
    Ok(digest(&manifest_bytes))
}
fn read_regular(path: &Path, max: u64) -> Result<Vec<u8>> {
    let meta = fs::symlink_metadata(path).map_err(|_| CaptureError("capture input missing"))?;
    if !meta.file_type().is_file() || meta.len() > max {
        return Err(CaptureError("capture input is not a bounded regular file"));
    }
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| file.take(max + 1).read_to_end(&mut bytes))
        .map_err(|_| CaptureError("capture input read failed"))?;
    if bytes.len() as u64 > max {
        return Err(CaptureError("capture input byte limit exceeded"));
    }
    Ok(bytes)
}
pub struct LoadedBundle {
    pub manifest: CaptureManifest,
    pub objects: Vec<(String, Vec<u8>)>,
    pub manifest_digest: String,
}
/// Supply the previously recorded manifest digest for whole-manifest tamper detection.
/// With None this checks structure and declared object integrity, not provenance authenticity.
pub fn load_bundle(
    path: &Path,
    expected_manifest_digest: Option<&str>,
    now_ms: u64,
) -> Result<LoadedBundle> {
    if !fs::symlink_metadata(path)
        .map_err(|_| CaptureError("capture directory missing"))?
        .file_type()
        .is_dir()
    {
        return Err(CaptureError("capture path is not a directory"));
    }
    if path
        .join("INCOMPLETE")
        .try_exists()
        .map_err(|_| CaptureError("capture marker cannot be checked"))?
    {
        return Err(CaptureError("capture is incomplete"));
    }
    let bytes = read_regular(&path.join("manifest.json"), MAX_MANIFEST_BYTES)?;
    let manifest_digest = digest(&bytes);
    if expected_manifest_digest.is_some_and(|x| x != manifest_digest) {
        return Err(CaptureError("manifest digest mismatch"));
    }
    let manifest: CaptureManifest = serde_json::from_slice(&bytes)
        .map_err(|_| CaptureError("manifest schema decode failed"))?;
    manifest.validate()?;
    if manifest.raw_expires_at_ms.is_some_and(|x| now_ms >= x) {
        return Err(CaptureError("capture raw retention expired"));
    }
    let mut objects = Vec::new();
    for object in &manifest.objects {
        let bytes = read_regular(&path.join(&object.name), object.bytes)?;
        if bytes.len() as u64 != object.bytes || digest(&bytes) != object.sha256 {
            return Err(CaptureError("capture object checksum mismatch"));
        }
        objects.push((object.name.clone(), bytes));
    }
    Ok(LoadedBundle {
        manifest,
        objects,
        manifest_digest,
    })
}

/// Hash the actual running binary without retaining it in memory.
pub fn file_digest(path: &Path) -> Result<String> {
    let mut file = File::open(path).map_err(|_| CaptureError("build identity cannot be read"))?;
    let mut hasher = Sha256::new();
    let mut bytes = [0; 65536];
    loop {
        let n = file
            .read(&mut bytes)
            .map_err(|_| CaptureError("build identity cannot be read"))?;
        if n == 0 {
            break;
        }
        hasher.update(&bytes[..n]);
    }
    Ok(format!("sha256:{}", hex::encode(hasher.finalize())))
}

#[cfg(test)]
mod tests {
    use super::*;
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    fn path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "arb-capture-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        ))
    }
    fn manifest() -> CaptureManifest {
        CaptureManifest {
            schema_version: 1,
            capture_id: "synthetic-test".into(),
            origin: Origin::Synthetic,
            network: Chain::BaseMainnet,
            provider_alias: "fixture".into(),
            adapter_version: "test-v1".into(),
            adapter_source_commit: "a".repeat(40),
            build_digest: digest(b"test"),
            config_digest: digest(b"config"),
            created_at_ms: 100,
            raw_expires_at_ms: Some(200),
            context: StateContext::Evm {
                block_number: 1,
                block_hash: format!("0x{}", "a".repeat(64)),
                parent_hash: format!("0x{}", "b".repeat(64)),
                block_timestamp_seconds: 1,
                finality: "finalized".into(),
            },
            first_sequence: 0,
            last_sequence: 0,
            required_inputs: vec!["tick-range".into()],
            missing_inputs: vec!["tick-range".into()],
            coherent: true,
            complete_for_quote: false,
            objects: vec![],
        }
    }
    fn write(path: &Path) -> String {
        write_bundle(
            path,
            manifest(),
            vec![("rpc.json".into(), b"{\"test\":true}".to_vec())],
            4096,
        )
        .unwrap()
    }
    #[test]
    fn durable_roundtrip_and_origin() {
        let p = path();
        let hash = write(&p);
        let b = load_bundle(&p, Some(&hash), 101).unwrap();
        assert!(!b.manifest.is_recorded_market_input());
        assert_eq!(b.objects.len(), 1);
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn rejects_corrupt_missing_and_expired() {
        let p = path();
        let hash = write(&p);
        assert!(load_bundle(&p, Some("wrong"), 101).is_err());
        assert!(load_bundle(&p, None, 200).is_err());
        fs::write(p.join("rpc.json"), b"{\"test\":fals}").unwrap();
        assert!(load_bundle(&p, Some(&hash), 101).is_err());
        fs::remove_file(p.join("rpc.json")).unwrap();
        assert!(load_bundle(&p, None, 101).is_err());
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn quota_failure_leaves_incomplete_and_no_overwrite() {
        let p = path();
        assert!(
            write_bundle(&p, manifest(), vec![("rpc.json".into(), b"{}".to_vec())], 1).is_err()
        );
        assert!(p.join("INCOMPLETE").exists());
        assert!(load_bundle(&p, None, 101).is_err());
        assert!(write_bundle(&p, manifest(), vec![], 4096).is_err());
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn rejects_unknown_schema_and_path_traversal() {
        let p = path();
        let mut m = manifest();
        m.schema_version = 2;
        assert!(write_bundle(&p, m, vec![("rpc.json".into(), b"{}".to_vec())], 4096).is_err());
        fs::remove_dir_all(p).unwrap();
        let p = path();
        assert!(
            write_bundle(
                &p,
                manifest(),
                vec![("../outside".into(), b"{}".to_vec())],
                4096
            )
            .is_err()
        );
        fs::remove_dir_all(p).unwrap();
    }

    #[test]
    fn digest_golden_vectors_preserve_prefix_lowercase_and_leading_zeros() {
        // Independent Python hashlib SHA-256 constants; never rewrite on upgrades.
        for (input, expected) in [
            (
                b"".as_slice(),
                "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                b"abc".as_slice(),
                "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ),
            (
                b"286".as_slice(),
                "sha256:00328ce57bbc14b33bd6695bc8eb32cdf2fb5f3a7d89ec14a42825e15d39df60",
            ),
            (
                b"\x00\x0f\x10\xff".as_slice(),
                "sha256:609a4c6f3ec6d8bdf3cf2589ce60b9b89049f7a5a519a0f2755dc43db323b161",
            ),
        ] {
            assert_eq!(digest(input), expected);
            assert!(valid_digest(expected));
        }
    }
    #[test]
    fn file_digest_preserves_independent_vectors_at_buffer_boundaries() {
        // Exercise empty input, the 64 KiB read boundary and multiple buffers.
        for (length, expected) in [
            (
                0_usize,
                "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                65_535_usize,
                "sha256:09ab7495d3e61a76f0deb12cb0306f0696cbb17ffc12131368c7a939f12f56d3",
            ),
            (
                65_536_usize,
                "sha256:1f8745f0d2d1387ec1af2211a3cf417b2e9e885e853472649c1d979d0e9370e3",
            ),
            (
                65_537_usize,
                "sha256:1abe08ebecf1c18cab71f6fe28aaddf20268f85bad78bb9a72f88ca47c874662",
            ),
            (
                131_073_usize,
                "sha256:0c5c5c759aa8164f9fb53c471ff060903c99edcb520fb7d9cb4bf7a45755f1c2",
            ),
        ] {
            let p = path();
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&p)
                .unwrap();
            file.write_all(&vec![b'x'; length]).unwrap();
            drop(file);
            let actual = file_digest(&p);
            fs::remove_file(p).unwrap();
            assert_eq!(actual.unwrap(), expected);
        }
    }
    #[test]
    fn missing_file_digest_remains_an_error_not_an_empty_hash() {
        assert_eq!(
            file_digest(&path()),
            Err(CaptureError("build identity cannot be read"))
        );
    }
}
