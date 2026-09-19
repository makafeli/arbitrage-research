//! Quota-pressure pruning for the raw capture volume.
//!
//! This is pressure relief so a worker survives a multi-day run on a fixed
//! quota, not a retention policy: pruning deletes the oldest committed
//! bundles (admitted ones included) before their `raw_expires_at_ms`, and it
//! runs before every collection attempt — research or readiness alike — so a
//! STOPPED or PAUSED session does not protect a bundle from it. The
//! `capture_admissions` rows (capture id, manifest digest) in PostgreSQL
//! record that a capture existed, but `scripts/export_capture_audit.py` only
//! audits presence; it copies nothing. To keep the raw bytes, copy the bundle
//! directory to `/data/archive` (a sibling of `/data/captures`, never pruned)
//! before pruning reaches it.
use arb_capture::{CaptureManifest, MAX_MANIFEST_BYTES};
use serde_json::json;
use std::{
    error::Error,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

type AnyError = Box<dyn Error + Send + Sync>;

const PRUNE_HIGH_WATERMARK_PERCENT: u64 = 80;
const PRUNE_LOW_WATERMARK_PERCENT: u64 = 50;
const PRUNE_MIN_AGE_MS: u64 = 600_000;

/// Bound the scan and reject symlinks. The initial deployment owns one capture volume
/// with one worker process; quota is shared across its retained old run directories.
pub(crate) fn directory_bytes(root: &Path) -> Result<u64, AnyError> {
    let mut paths = vec![(root.to_owned(), 0)];
    let mut entries = 0_u32;
    let mut total = 0_u64;
    while let Some((path, depth)) = paths.pop() {
        if depth > 4 {
            return Err("capture directory nesting exceeds supported layout".into());
        }
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            entries = entries
                .checked_add(1)
                .ok_or("capture directory count overflow")?;
            if entries > 100_000 {
                return Err("capture directory entry quota exceeded".into());
            }
            let meta = fs::symlink_metadata(entry.path())?;
            if meta.file_type().is_symlink() {
                return Err("capture volume contains symlink".into());
            }
            if meta.is_dir() {
                paths.push((entry.path(), depth + 1));
            } else if meta.is_file() {
                total = total
                    .checked_add(meta.len())
                    .ok_or("capture byte count overflow")?;
            } else {
                return Err("capture volume contains unsupported file type".into());
            }
        }
    }
    Ok(total)
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PruneSummary {
    pub deleted: u32,
    pub bytes_freed: u64,
    pub used_after: u64,
    pub skipped_in_flight: u32,
    pub skipped_young: u32,
    pub skipped_unreadable: u32,
}

struct Candidate {
    capture_id: String,
    path: PathBuf,
    created_at_ms: u64,
}

/// `manifest.json` is read directly with the `arb_capture` manifest type rather than
/// hand-parsed; any read or decode failure is treated as an unreadable candidate.
fn read_manifest(bundle: &Path) -> Option<CaptureManifest> {
    let mut file = fs::File::open(bundle.join("manifest.json")).ok()?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return None;
    }
    serde_json::from_slice::<CaptureManifest>(&bytes).ok()
}

/// Prunes the oldest committed capture bundles once `root` crosses the high
/// watermark (80% of quota), deleting from the oldest until usage is back at
/// or below the low watermark (50% of quota). In-flight bundles (an
/// `INCOMPLETE` marker present), bundles younger than `PRUNE_MIN_AGE_MS`, and
/// bundles whose manifest cannot be read are never deleted. A single removal
/// failure is logged and pruning continues with the remaining candidates.
pub(crate) fn prune_under_quota_pressure(
    root: &Path,
    quota_bytes: u64,
    now_ms: u64,
) -> Result<PruneSummary, AnyError> {
    let mut used = directory_bytes(root)?;
    let high_watermark = quota_bytes / 100 * PRUNE_HIGH_WATERMARK_PERCENT;
    if used <= high_watermark {
        return Ok(PruneSummary::default());
    }
    let low_watermark = quota_bytes / 100 * PRUNE_LOW_WATERMARK_PERCENT;
    let mut candidates = Vec::new();
    let mut skipped_in_flight = 0_u32;
    let mut skipped_young = 0_u32;
    let mut skipped_unreadable = 0_u32;
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path)?;
        if meta.file_type().is_symlink() || !meta.is_dir() {
            continue;
        }
        if path.join("INCOMPLETE").exists() {
            skipped_in_flight += 1;
            continue;
        }
        let manifest = match read_manifest(&path) {
            Some(manifest) => manifest,
            None => {
                skipped_unreadable += 1;
                continue;
            }
        };
        if now_ms.saturating_sub(manifest.created_at_ms) < PRUNE_MIN_AGE_MS {
            skipped_young += 1;
            continue;
        }
        candidates.push(Candidate {
            capture_id: manifest.capture_id,
            path,
            created_at_ms: manifest.created_at_ms,
        });
    }
    candidates.sort_by_key(|candidate| candidate.created_at_ms);
    let mut deleted = 0_u32;
    let mut bytes_freed = 0_u64;
    for candidate in candidates {
        if used <= low_watermark {
            break;
        }
        // Measured per bundle right before deletion; `used` is then adjusted
        // locally so the whole root is never re-walked per deletion. A failed
        // measurement is treated the same as a failed removal: log and move
        // on to the next candidate rather than aborting the whole pass.
        let bytes = match directory_bytes(&candidate.path) {
            Ok(bytes) => bytes,
            Err(error) => {
                println!(
                    "{}",
                    json!({
                        "event": "capture-prune-failed",
                        "capture_id": candidate.capture_id,
                        "error": error.to_string(),
                    })
                );
                continue;
            }
        };
        match fs::remove_dir_all(&candidate.path) {
            Ok(()) => {
                used = used.saturating_sub(bytes);
                bytes_freed = bytes_freed.saturating_add(bytes);
                deleted += 1;
                println!(
                    "{}",
                    json!({
                        "event": "capture-pruned",
                        "capture_id": candidate.capture_id,
                        "bytes": bytes,
                        "created_at_ms": candidate.created_at_ms,
                        "reason": "quota-pressure",
                    })
                );
            }
            Err(error) => {
                // `remove_dir_all` can remove some files before hitting one it
                // can't; re-measure what is left so `used`/`bytes_freed` stay
                // honest instead of assuming the whole bundle survived. If the
                // re-measurement itself fails, treat it as nothing freed.
                let remaining = directory_bytes(&candidate.path).unwrap_or(bytes);
                let freed = bytes.saturating_sub(remaining);
                used = used.saturating_sub(freed);
                bytes_freed = bytes_freed.saturating_add(freed);
                println!(
                    "{}",
                    json!({
                        "event": "capture-prune-failed",
                        "capture_id": candidate.capture_id,
                        "error": error.kind().to_string(),
                    })
                );
            }
        }
    }
    let summary = PruneSummary {
        deleted,
        bytes_freed,
        used_after: used,
        skipped_in_flight,
        skipped_young,
        skipped_unreadable,
    };
    println!(
        "{}",
        json!({
            "event": "capture-prune-summary",
            "deleted": summary.deleted,
            "bytes_freed": summary.bytes_freed,
            "used_after": summary.used_after,
            "quota_bytes": quota_bytes,
            "skipped_in_flight": summary.skipped_in_flight,
            "skipped_young": summary.skipped_young,
            "skipped_unreadable": summary.skipped_unreadable,
        })
    );
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arb_adapter_api::{Chain, StateContext};
    use arb_capture::{Origin, write_bundle};
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn scratch_root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "arb-worker-prune-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ))
    }

    fn manifest(capture_id: &str, created_at_ms: u64) -> CaptureManifest {
        CaptureManifest {
            schema_version: 1,
            capture_id: capture_id.into(),
            origin: Origin::Synthetic,
            network: Chain::BaseMainnet,
            provider_alias: "fixture".into(),
            adapter_version: "test-v1".into(),
            adapter_source_commit: "a".repeat(40),
            build_digest: arb_capture::digest(b"test"),
            config_digest: arb_capture::digest(b"config"),
            created_at_ms,
            raw_expires_at_ms: Some(created_at_ms + 1_000_000_000),
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

    fn bundle_with_payload_len(
        root: &Path,
        capture_id: &str,
        created_at_ms: u64,
        payload_len: usize,
    ) -> PathBuf {
        let path = root.join(capture_id);
        write_bundle(
            &path,
            manifest(capture_id, created_at_ms),
            vec![("rpc.json".into(), vec![7_u8; payload_len])],
            (payload_len as u64 + 4096).max(1024 * 1024),
        )
        .unwrap();
        path
    }

    /// Finds the smallest payload length (>= 1, since `write_bundle` rejects
    /// a zero-byte object) whose committed bundle has a total byte size
    /// congruent to `remainder` modulo `modulus`, and returns that bundle
    /// alongside its exact size. Searching real, on-disk bundle sizes rather
    /// than reasoning about manifest.json's exact byte cost sidesteps the
    /// fact that the embedded object-size field changes the manifest's own
    /// length by a byte or two as its own digit count grows.
    fn bundle_with_size_congruent_to(
        root: &Path,
        capture_id: &str,
        created_at_ms: u64,
        modulus: u64,
        remainder: u64,
    ) -> (PathBuf, u64) {
        let mut payload_len = 1_usize;
        loop {
            assert!(
                payload_len < 10_000,
                "no matching bundle size found in range"
            );
            let path = bundle_with_payload_len(root, capture_id, created_at_ms, payload_len);
            let total = directory_bytes(&path).unwrap();
            if total % modulus == remainder {
                return (path, total);
            }
            fs::remove_dir_all(&path).unwrap();
            payload_len += 1;
        }
    }

    fn bundle(root: &Path, capture_id: &str, created_at_ms: u64) -> PathBuf {
        bundle_with_payload_len(root, capture_id, created_at_ms, 1024)
    }

    #[test]
    fn below_watermark_prunes_nothing_and_writes_nothing() {
        let root = scratch_root();
        fs::create_dir(&root).unwrap();
        bundle(&root, "cap-a", 1_000);
        let before = fs::metadata(root.join("cap-a"))
            .unwrap()
            .modified()
            .unwrap();
        let summary = prune_under_quota_pressure(&root, 1024 * 1024 * 1024, 2_000_000).unwrap();
        assert_eq!(summary, PruneSummary::default());
        assert!(root.join("cap-a").exists());
        assert_eq!(
            fs::metadata(root.join("cap-a"))
                .unwrap()
                .modified()
                .unwrap(),
            before
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn above_watermark_deletes_oldest_first_until_low_watermark() {
        let root = scratch_root();
        fs::create_dir(&root).unwrap();
        // Five identically-shaped committed bundles (same id length and same
        // digit width in every timestamp field), so each is the same size on
        // disk and the watermark math below is exact.
        let ids = ["cap-1", "cap-2", "cap-3", "cap-4", "cap-5"];
        let ages = [1_000, 2_000, 3_000, 4_000, 5_000];
        for (id, age) in ids.iter().zip(ages) {
            bundle(&root, id, age);
        }
        let unit = directory_bytes(&root.join("cap-1")).unwrap();
        let quota_bytes = unit * 5;
        let now_ms = 5_000 + PRUNE_MIN_AGE_MS; // all five are old enough
        let summary = prune_under_quota_pressure(&root, quota_bytes, now_ms).unwrap();
        // high watermark ~= 4*unit (5*unit used triggers pruning), low watermark ~=
        // 2.5*unit, so oldest-first deletion stops after the third bundle, at 2*unit.
        assert!(!root.join("cap-1").exists(), "oldest must be pruned first");
        assert!(!root.join("cap-2").exists());
        assert!(!root.join("cap-3").exists());
        assert!(root.join("cap-4").exists(), "newer bundles must be kept");
        assert!(root.join("cap-5").exists(), "newest bundle must be kept");
        assert_eq!(summary.deleted, 3);
        assert_eq!(summary.bytes_freed, unit * 3);
        assert_eq!(summary.used_after, unit * 2);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn skips_in_flight_young_and_unreadable_candidates() {
        let root = scratch_root();
        fs::create_dir(&root).unwrap();
        let old_committed = bundle(&root, "cap-old", 1_000);
        let young_committed = bundle(&root, "cap-young", 9_000);
        let incomplete = root.join("cap-incomplete");
        fs::create_dir(&incomplete).unwrap();
        fs::write(incomplete.join("INCOMPLETE"), b"capture not committed\n").unwrap();
        let unreadable = root.join("cap-unreadable");
        fs::create_dir(&unreadable).unwrap();
        fs::write(unreadable.join("manifest.json"), b"not json").unwrap();
        // cap-old is exactly PRUNE_MIN_AGE_MS old (eligible); cap-young is not.
        let now_ms = 1_000 + PRUNE_MIN_AGE_MS;
        let summary = prune_under_quota_pressure(&root, 1, now_ms).unwrap();
        assert!(
            !old_committed.exists(),
            "old committed bundle must be pruned"
        );
        assert!(young_committed.exists(), "young bundle must be kept");
        assert!(incomplete.exists(), "in-flight bundle must be kept");
        assert!(unreadable.exists(), "unreadable bundle must be kept");
        assert_eq!(summary.deleted, 1);
        assert_eq!(summary.skipped_in_flight, 1);
        assert_eq!(summary.skipped_young, 1);
        assert_eq!(summary.skipped_unreadable, 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn removal_failure_is_reported_and_pruning_continues() {
        use std::os::unix::fs::PermissionsExt;
        let root = scratch_root();
        fs::create_dir(&root).unwrap();
        let blocked = bundle(&root, "cap-blocked", 1_000);
        let also_old = bundle(&root, "cap-also-old", 1_500);
        let blocked_bytes = directory_bytes(&blocked).unwrap();
        let also_old_bytes = directory_bytes(&also_old).unwrap();
        // Removing entries from a directory needs write permission on that
        // directory itself; strip it so `remove_dir_all` fails for `blocked`.
        fs::set_permissions(&blocked, fs::Permissions::from_mode(0o555)).unwrap();
        let now_ms = 1_500 + PRUNE_MIN_AGE_MS;
        let summary = prune_under_quota_pressure(&root, 1_000, now_ms).unwrap();
        // Restore permissions before cleanup regardless of outcome.
        fs::set_permissions(&blocked, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(
            blocked.exists(),
            "a failed removal must leave the bundle in place"
        );
        assert!(
            !also_old.exists(),
            "pruning must continue past a removal failure"
        );
        assert_eq!(summary.deleted, 1);
        // `blocked`'s permission denies removing anything under it, so
        // nothing was actually freed for it; the re-measurement after the
        // failed `remove_dir_all` must reflect that rather than assuming the
        // whole bundle was removed.
        assert_eq!(
            directory_bytes(&blocked).unwrap(),
            blocked_bytes,
            "the blocked bundle must be untouched"
        );
        assert_eq!(
            summary.bytes_freed, also_old_bytes,
            "bytes_freed must count only what was actually removed"
        );
        assert_eq!(summary.used_after, blocked_bytes);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn watermark_constants_are_pinned() {
        // A silent change to any of these three numbers changes the worker's
        // survival behaviour under quota pressure; pin them so that change is
        // deliberate, not accidental.
        assert_eq!(PRUNE_HIGH_WATERMARK_PERCENT, 80);
        assert_eq!(PRUNE_LOW_WATERMARK_PERCENT, 50);
        assert_eq!(PRUNE_MIN_AGE_MS, 600_000);
    }

    #[test]
    fn used_exactly_at_high_watermark_prunes_nothing() {
        let root = scratch_root();
        fs::create_dir(&root).unwrap();
        // A quota chosen as `(total / 80) * 100` makes `quota / 100 * 80`
        // reproduce `total` exactly, with no rounding error either way, as
        // long as `total` is itself an exact multiple of 80.
        let (path, total) = bundle_with_size_congruent_to(&root, "cap-exact", 1_000, 80, 0);
        let quota_bytes = total / 80 * 100;
        let now_ms = 1_000 + PRUNE_MIN_AGE_MS;
        let summary = prune_under_quota_pressure(&root, quota_bytes, now_ms).unwrap();
        assert_eq!(
            summary,
            PruneSummary::default(),
            "used exactly at the high watermark must not prune"
        );
        assert!(path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn one_byte_above_high_watermark_triggers_pruning() {
        let root = scratch_root();
        fs::create_dir(&root).unwrap();
        // `total` is one byte above a multiple of 80, so the quota derived
        // from that multiple (`total - 1`) puts `used` exactly one byte over
        // the high watermark.
        let (path, total) = bundle_with_size_congruent_to(&root, "cap-over", 1_000, 80, 1);
        let quota_bytes = (total - 1) / 80 * 100;
        let now_ms = 1_000 + PRUNE_MIN_AGE_MS;
        let summary = prune_under_quota_pressure(&root, quota_bytes, now_ms).unwrap();
        assert_eq!(
            summary.deleted, 1,
            "one byte over the high watermark must trigger pruning"
        );
        assert!(!path.exists());
        fs::remove_dir_all(root).unwrap();
    }
}
