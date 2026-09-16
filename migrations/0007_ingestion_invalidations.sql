-- Base stream continuity only. This does not qualify ticks, freshness or quotes.
-- A single terminal invalidation fences every batch of the same immutable stream.
CREATE TABLE ingestion_invalidations (
 operator_id text NOT NULL,
 stream_id text NOT NULL,
 schema_version integer NOT NULL DEFAULT 1 CHECK(schema_version=1),
 policy_version text NOT NULL DEFAULT 'base-finalized-stream-v1'
  CHECK(policy_version='base-finalized-stream-v1'),
 invalidation_revision bigint NOT NULL CHECK(invalidation_revision>0),
 reason text NOT NULL CHECK(reason IN
  ('CONTINUITY_LOST','PROVIDER_FAILURE','RESOURCE_LIMIT','INVALID_INPUT')),
 binding jsonb NOT NULL CHECK(jsonb_typeof(binding)='object'),
 checkpoint jsonb NOT NULL CHECK(jsonb_typeof(checkpoint)='object'),
 source text NOT NULL CHECK(source IN ('STREAM_TRANSITION','MIGRATED_HALT')),
 detected_at timestamptz NOT NULL,
 recorded_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(operator_id,stream_id),
 FOREIGN KEY(operator_id,stream_id) REFERENCES ingestion_streams(operator_id,stream_id)
);

CREATE FUNCTION validate_ingestion_invalidation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NOT EXISTS (
  SELECT 1 FROM ingestion_streams s
  WHERE s.operator_id=NEW.operator_id AND s.stream_id=NEW.stream_id
   AND s.state='HALTED' AND s.revision=NEW.invalidation_revision
   AND s.halt_reason=NEW.reason AND s.binding=NEW.binding
   AND s.checkpoint=NEW.checkpoint AND s.updated_at=NEW.detected_at
   AND s.binding->>'network_id'='base-mainnet'
 ) THEN
  RAISE EXCEPTION 'invalidation does not match a halted Base stream';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER valid_ingestion_invalidation BEFORE INSERT ON ingestion_invalidations
 FOR EACH ROW EXECUTE FUNCTION validate_ingestion_invalidation();
CREATE TRIGGER append_only_ingestion_invalidations BEFORE UPDATE OR DELETE ON ingestion_invalidations
 FOR EACH ROW EXECUTE FUNCTION reject_audit_mutation();

CREATE FUNCTION record_ingestion_invalidation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 INSERT INTO ingestion_invalidations
  (operator_id,stream_id,invalidation_revision,reason,binding,checkpoint,source,detected_at)
 VALUES
  (NEW.operator_id,NEW.stream_id,NEW.revision,NEW.halt_reason,NEW.binding,
   NEW.checkpoint,'STREAM_TRANSITION',NEW.updated_at);
 RETURN NEW;
END $$;
CREATE TRIGGER journal_ingestion_invalidation AFTER UPDATE ON ingestion_streams
 FOR EACH ROW WHEN (OLD.state='ACTIVE' AND NEW.state='HALTED')
 EXECUTE FUNCTION record_ingestion_invalidation();

-- Preserve the old detection timestamp. recorded_at is the migration time, not
-- a newly observed rollback. No existing cursor, batch or capture is rewritten.
INSERT INTO ingestion_invalidations
 (operator_id,stream_id,invalidation_revision,reason,binding,checkpoint,source,detected_at)
 SELECT operator_id,stream_id,revision,halt_reason,binding,checkpoint,
  'MIGRATED_HALT',updated_at
 FROM ingestion_streams WHERE state='HALTED' AND binding->>'network_id'='base-mainnet';

CREATE VIEW ingestion_snapshot_validity AS
 SELECT b.operator_id,b.stream_id,b.revision AS snapshot_revision,
  b.payload_digest AS snapshot_digest,b.checkpoint AS snapshot_checkpoint,
  s.binding,s.revision AS cursor_revision,s.state AS stream_state,
  'base-finalized-stream-v1'::text AS policy_version,
  CASE
   WHEN s.binding->>'network_id' IS DISTINCT FROM 'base-mainnet'
    THEN 'UNVERIFIABLE'
   WHEN s.state='ACTIVE' AND i.stream_id IS NULL AND b.revision<=s.revision
    THEN 'NO_KNOWN_INVALIDATION'
   WHEN s.state='HALTED' AND i.invalidation_revision=s.revision
    AND i.reason=s.halt_reason AND i.binding=s.binding AND i.checkpoint=s.checkpoint
    AND i.detected_at=s.updated_at AND b.revision<i.invalidation_revision
    THEN 'INVALIDATED'
   ELSE 'UNVERIFIABLE'
  END AS continuity_status,
  i.invalidation_revision,i.reason AS invalidation_reason,
  i.source AS invalidation_source,i.detected_at,i.recorded_at
 FROM ingestion_batches b
 JOIN ingestion_streams s USING(operator_id,stream_id)
 LEFT JOIN ingestion_invalidations i USING(operator_id,stream_id);

-- Internal, invoker-rights gate for a SHORT caller-owned transaction. The SHARE
-- lock serializes against HALT and advancement until that transaction ends.
-- A returned payload is not a transferable authorization or a complete quote.
CREATE FUNCTION require_current_ingestion_snapshot(
 p_operator text,p_stream text,p_revision bigint,p_digest text
) RETURNS jsonb LANGUAGE plpgsql SET lock_timeout='2s' AS $$
DECLARE
 s ingestion_streams%ROWTYPE;
 b ingestion_batches%ROWTYPE;
BEGIN
 IF p_operator IS NULL OR length(p_operator) NOT BETWEEN 1 AND 128
  OR p_operator !~ '^[A-Za-z0-9_.:-]+$'
  OR p_stream IS NULL OR length(p_stream) NOT BETWEEN 1 AND 128
  OR p_stream !~ '^[A-Za-z0-9_.:-]+$'
  OR p_revision IS NULL OR p_revision NOT BETWEEN 1 AND 4096
  OR p_digest IS NULL OR p_digest !~ '^sha256:[0-9a-f]{64}$'
 THEN
  RAISE EXCEPTION USING ERRCODE='22023',MESSAGE='invalid snapshot reference';
 END IF;
 SELECT * INTO s FROM ingestion_streams
  WHERE operator_id=p_operator AND stream_id=p_stream FOR SHARE;
 IF NOT FOUND THEN
  RAISE EXCEPTION USING ERRCODE='P0002',MESSAGE='snapshot not found';
 END IF;
 IF s.binding->>'network_id' IS DISTINCT FROM 'base-mainnet'
  OR s.state<>'ACTIVE'
  OR EXISTS(SELECT 1 FROM ingestion_invalidations
   WHERE operator_id=p_operator AND stream_id=p_stream)
 THEN
  RAISE EXCEPTION USING ERRCODE='55000',MESSAGE='snapshot continuity unavailable';
 END IF;
 SELECT * INTO b FROM ingestion_batches
  WHERE operator_id=p_operator AND stream_id=p_stream AND revision=p_revision;
 IF NOT FOUND THEN
  RAISE EXCEPTION USING ERRCODE='P0002',MESSAGE='snapshot not found';
 END IF;
 IF b.payload_digest<>p_digest OR b.revision>s.revision THEN
  RAISE EXCEPTION USING ERRCODE='22023',MESSAGE='snapshot reference mismatch';
 END IF;
 RETURN b.payload;
END $$;
