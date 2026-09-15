//! Atomic, bounded event-batch persistence. This is not quote qualification.
use crate::{Store, StoreError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{Row, postgres::PgRow};

const MAX_BATCH_BYTES: usize = 2 * 1024 * 1024;
const MAX_STREAM_BYTES: i64 = 64 * 1024 * 1024;
const MAX_BATCHES: u64 = 4096;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IngestionHead {
    pub number: u64,
    pub hash: String,
    pub parent_hash: String,
    pub timestamp_seconds: u64,
}
impl IngestionHead {
    fn validate(&self) -> Result<(), StoreError> {
        if !hex_id(&self.hash, "0x", 64) || !hex_id(&self.parent_hash, "0x", 64) {
            return Err(StoreError::InvalidInput("invalid ingestion header"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IngestionBinding {
    pub schema_version: u32,
    pub network_id: String,
    pub registry_digest: String,
    pub abi_source_commit: String,
    pub dataset_origin: String,
    pub pool_addresses: Vec<String>,
}
impl IngestionBinding {
    pub fn validate(&self) -> Result<(), StoreError> {
        if self.schema_version != 1
            || self.network_id != "base-mainnet"
            || !hex_id(&self.registry_digest, "sha256:", 64)
            || !hex_id(&self.abi_source_commit, "", 40)
            || !matches!(
                self.dataset_origin.as_str(),
                "RECORDED_LIVE" | "MANUALLY_CONSTRUCTED"
            )
            || self.pool_addresses.is_empty()
            || self.pool_addresses.len() > 8
            || self.pool_addresses.iter().any(|p| !hex_id(p, "0x", 40))
            || self.pool_addresses.windows(2).any(|p| p[0] >= p[1])
        {
            return Err(StoreError::InvalidInput("invalid ingestion binding"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IngestionCursor {
    pub binding: IngestionBinding,
    pub checkpoint: IngestionHead,
    pub revision: u64,
    pub state: String,
    pub halt_reason: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub enum IngestionHalt {
    ContinuityLost,
    ProviderFailure,
    ResourceLimit,
    InvalidInput,
}
impl IngestionHalt {
    fn name(self) -> &'static str {
        match self {
            Self::ContinuityLost => "CONTINUITY_LOST",
            Self::ProviderFailure => "PROVIDER_FAILURE",
            Self::ResourceLimit => "RESOURCE_LIMIT",
            Self::InvalidInput => "INVALID_INPUT",
        }
    }
}

fn hex_id(value: &str, prefix: &str, digits: usize) -> bool {
    value.strip_prefix(prefix).is_some_and(|s| {
        s.len() == digits
            && s.bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    })
}
fn identity(operator: &str, stream: &str) -> Result<(), StoreError> {
    for value in [operator, stream] {
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_.:".contains(&c))
        {
            return Err(StoreError::InvalidInput("invalid ingestion identity"));
        }
    }
    Ok(())
}
fn cursor(row: &PgRow) -> Result<IngestionCursor, StoreError> {
    let binding: IngestionBinding =
        serde_json::from_value(row.try_get("binding")?).map_err(|_| StoreError::CorruptState)?;
    let checkpoint: IngestionHead =
        serde_json::from_value(row.try_get("checkpoint")?).map_err(|_| StoreError::CorruptState)?;
    binding.validate().map_err(|_| StoreError::CorruptState)?;
    checkpoint
        .validate()
        .map_err(|_| StoreError::CorruptState)?;
    Ok(IngestionCursor {
        binding,
        checkpoint,
        revision: u64::try_from(row.try_get::<i64, _>("revision")?)
            .map_err(|_| StoreError::CorruptState)?,
        state: row.try_get("state")?,
        halt_reason: row.try_get("halt_reason")?,
    })
}
fn as_value(value: &impl Serialize) -> Result<Value, StoreError> {
    serde_json::to_value(value).map_err(|_| StoreError::InvalidInput("invalid ingestion data"))
}

/// Validate the durable envelope again; the caller still owns full ABI/contract
/// validation. Accepted payloads are decoded projections, never private RPC errors.
fn validate_batch(
    binding: &IngestionBinding,
    from: &IngestionHead,
    value: &Value,
) -> Result<(IngestionHead, Vec<u8>), StoreError> {
    binding.validate()?;
    from.validate()?;
    let invalid = || StoreError::InvalidInput("invalid ingestion batch");
    if value.as_object().is_none_or(|o| o.len() != 9)
        || value["schema_version"] != 1
        || value["network_id"] != binding.network_id
        || value["registry_digest"] != binding.registry_digest
        || value["abi_source_commit"] != binding.abi_source_commit
        || value["pool_addresses"] != as_value(&binding.pool_addresses)?
        || value["from_checkpoint"] != as_value(from)?
        || value["full_snapshot_required"] != true
    {
        return Err(invalid());
    }
    let through: IngestionHead =
        serde_json::from_value(value["through"].clone()).map_err(|_| invalid())?;
    through.validate()?;
    let blocks = value["blocks"]
        .as_array()
        .filter(|b| b.len() <= 32)
        .ok_or_else(invalid)?;
    let bytes = serde_json::to_vec(value).map_err(|_| invalid())?;
    if bytes.len() > MAX_BATCH_BYTES {
        return Err(invalid());
    }
    let mut previous = from.clone();
    let mut count = 0usize;
    for block in blocks {
        let head: IngestionHead =
            serde_json::from_value(block["header"].clone()).map_err(|_| invalid())?;
        head.validate()?;
        if previous.number.checked_add(1) != Some(head.number)
            || head.parent_hash != previous.hash
            || head.timestamp_seconds < previous.timestamp_seconds
        {
            return Err(invalid());
        }
        let logs = block["logs"]
            .as_array()
            .filter(|l| l.len() <= 512)
            .ok_or_else(invalid)?;
        count += logs.len();
        if count > 4096 {
            return Err(invalid());
        }
        let mut last = None;
        for log in logs {
            let index = log["log_index"].as_u64().ok_or_else(invalid)?;
            if !log["pool"]
                .as_str()
                .is_some_and(|p| binding.pool_addresses.iter().any(|a| a == p))
                || !log["transaction_hash"]
                    .as_str()
                    .is_some_and(|h| hex_id(h, "0x", 64))
                || log["transaction_index"].as_u64().is_none()
                || !log["decoded"].is_object()
                || log.as_object().is_none_or(|o| o.len() != 8)
                || log["block_number"].as_u64() != Some(head.number)
                || log["block_hash"] != head.hash
                || log["removed"] != false
                || last.is_some_and(|old| old >= index)
            {
                return Err(invalid());
            }
            last = Some(index);
        }
        previous = head;
    }
    if previous != through {
        return Err(invalid());
    }
    Ok((through, bytes))
}

impl Store {
    /// Explicit initialization only. The original binding and seed are immutable;
    /// a retry cannot silently replace coverage with a newly observed chain tip.
    pub async fn create_ingestion(
        &self,
        operator: &str,
        stream: &str,
        binding: &IngestionBinding,
        initial: &IngestionHead,
    ) -> Result<IngestionCursor, StoreError> {
        identity(operator, stream)?;
        binding.validate()?;
        initial.validate()?;
        let binding = as_value(binding)?;
        let initial = as_value(initial)?;
        sqlx::query("INSERT INTO ingestion_streams(operator_id,stream_id,binding,initial_checkpoint,checkpoint) VALUES($1,$2,$3,$4,$4) ON CONFLICT DO NOTHING")
            .bind(operator).bind(stream).bind(&binding).bind(&initial).execute(&self.pool).await?;
        let row =
            sqlx::query("SELECT * FROM ingestion_streams WHERE operator_id=$1 AND stream_id=$2")
                .bind(operator)
                .bind(stream)
                .fetch_one(&self.pool)
                .await?;
        if row.try_get::<Value, _>("binding")? != binding
            || row.try_get::<Value, _>("initial_checkpoint")? != initial
        {
            return Err(StoreError::Conflict("ingestion initialization changed"));
        }
        cursor(&row)
    }

    pub async fn ingestion_cursor(
        &self,
        operator: &str,
        stream: &str,
    ) -> Result<IngestionCursor, StoreError> {
        identity(operator, stream)?;
        let row =
            sqlx::query("SELECT * FROM ingestion_streams WHERE operator_id=$1 AND stream_id=$2")
                .bind(operator)
                .bind(stream)
                .fetch_optional(&self.pool)
                .await?
                .ok_or(StoreError::NotFound)?;
        cursor(&row)
    }

    /// Atomically append the whole recovery batch and advance the cursor. A retry
    /// after uncertain COMMIT is idempotent only for the identical source batch.
    pub async fn commit_ingestion(
        &self,
        operator: &str,
        stream: &str,
        expected: &IngestionCursor,
        batch: Value,
    ) -> Result<IngestionCursor, StoreError> {
        identity(operator, stream)?;
        let (through, bytes) = validate_batch(&expected.binding, &expected.checkpoint, &batch)?;
        let digest = format!("sha256:{}", hex::encode(Sha256::digest(&bytes)));
        let revision = expected
            .revision
            .checked_add(1)
            .filter(|v| *v <= MAX_BATCHES)
            .ok_or(StoreError::InvalidInput(
                "ingestion retention limit reached",
            ))? as i64;
        let mut tx = self.pool.begin().await?;
        sqlx::query("SET LOCAL lock_timeout='2s'")
            .execute(&mut *tx)
            .await?;
        sqlx::query("SET LOCAL statement_timeout='10s'")
            .execute(&mut *tx)
            .await?;
        let row = sqlx::query(
            "SELECT * FROM ingestion_streams WHERE operator_id=$1 AND stream_id=$2 FOR UPDATE",
        )
        .bind(operator)
        .bind(stream)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(StoreError::NotFound)?;
        let current = cursor(&row)?;
        if current.binding != expected.binding {
            return Err(StoreError::Conflict("ingestion binding changed"));
        }
        if current.state != "ACTIVE" {
            return Err(StoreError::Conflict("halted ingestion cursor"));
        }
        if let Some(saved)=sqlx::query_scalar::<_,String>("SELECT payload_digest FROM ingestion_batches WHERE operator_id=$1 AND stream_id=$2 AND revision=$3")
            .bind(operator).bind(stream).bind(revision).fetch_optional(&mut *tx).await? {
            return if saved==digest { Ok(current) } else { Err(StoreError::Conflict("ingestion retry payload changed")) };
        }
        if current != *expected || current.state != "ACTIVE" {
            return Err(StoreError::Conflict("stale or halted ingestion cursor"));
        }
        if through == expected.checkpoint {
            return Ok(current);
        }
        let retained = row
            .try_get::<i64, _>("retained_bytes")?
            .checked_add(bytes.len() as i64)
            .filter(|n| *n <= MAX_STREAM_BYTES)
            .ok_or(StoreError::InvalidInput(
                "ingestion retention limit reached",
            ))?;
        let end = as_value(&through)?;
        sqlx::query("INSERT INTO ingestion_batches(operator_id,stream_id,revision,payload_digest,payload,payload_bytes,checkpoint) VALUES($1,$2,$3,$4,$5,$6,$7)")
            .bind(operator).bind(stream).bind(revision).bind(&digest).bind(batch).bind(bytes.len() as i64).bind(&end).execute(&mut *tx).await?;
        let row=sqlx::query("UPDATE ingestion_streams SET checkpoint=$3,revision=$4,retained_bytes=$5,updated_at=clock_timestamp() WHERE operator_id=$1 AND stream_id=$2 RETURNING *")
            .bind(operator).bind(stream).bind(end).bind(revision).bind(retained).fetch_one(&mut *tx).await?;
        let result = cursor(&row)?;
        tx.commit().await?;
        Ok(result)
    }

    /// Persist a terminal gap at the exact cursor, without advancing or re-arming.
    pub async fn halt_ingestion(
        &self,
        operator: &str,
        stream: &str,
        expected: &IngestionCursor,
        reason: IngestionHalt,
    ) -> Result<IngestionCursor, StoreError> {
        identity(operator, stream)?;
        let revision = i64::try_from(expected.revision)
            .map_err(|_| StoreError::InvalidInput("invalid revision"))?;
        let row=sqlx::query("UPDATE ingestion_streams SET state='HALTED',halt_reason=$3,revision=revision+1,updated_at=clock_timestamp() WHERE operator_id=$1 AND stream_id=$2 AND revision=$4 AND state='ACTIVE' AND binding=$5 AND checkpoint=$6 RETURNING *")
            .bind(operator).bind(stream).bind(reason.name()).bind(revision)
            .bind(as_value(&expected.binding)?).bind(as_value(&expected.checkpoint)?)
            .fetch_optional(&self.pool).await?.ok_or(StoreError::Conflict("stale or halted ingestion cursor"))?;
        cursor(&row)
    }

    /// Bounded diagnostic reading, independent of market completeness claims.
    pub async fn ingestion_batches(
        &self,
        operator: &str,
        stream: &str,
        after: u64,
        limit: u32,
    ) -> Result<Vec<Value>, StoreError> {
        identity(operator, stream)?;
        if !(1..=16).contains(&limit) {
            return Err(StoreError::InvalidInput("invalid batch page limit"));
        }
        let after =
            i64::try_from(after).map_err(|_| StoreError::InvalidInput("invalid batch revision"))?;
        sqlx::query_scalar("SELECT payload FROM ingestion_batches WHERE operator_id=$1 AND stream_id=$2 AND revision>$3 ORDER BY revision LIMIT $4")
            .bind(operator).bind(stream).bind(after).bind(i64::from(limit)).fetch_all(&self.pool).await.map_err(StoreError::from)
    }
}
