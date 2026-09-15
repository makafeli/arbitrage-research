-- Research-only event journal. Initial coverage is explicitly after the seed.
-- Batches and cursor advancement commit atomically; a halted stream cannot re-arm.
CREATE TABLE ingestion_streams (
 operator_id text NOT NULL, stream_id text NOT NULL,
 binding jsonb NOT NULL CHECK(jsonb_typeof(binding)='object'),
 initial_checkpoint jsonb NOT NULL CHECK(jsonb_typeof(initial_checkpoint)='object'),
 checkpoint jsonb NOT NULL CHECK(jsonb_typeof(checkpoint)='object'),
 revision bigint NOT NULL DEFAULT 0 CHECK(revision>=0),
 state text NOT NULL DEFAULT 'ACTIVE' CHECK(state IN ('ACTIVE','HALTED')),
 halt_reason text CHECK(halt_reason IN ('CONTINUITY_LOST','PROVIDER_FAILURE','RESOURCE_LIMIT','INVALID_INPUT')),
 retained_bytes bigint NOT NULL DEFAULT 0 CHECK(retained_bytes BETWEEN 0 AND 67108864),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(operator_id,stream_id),
 CHECK((state='ACTIVE' AND halt_reason IS NULL) OR (state='HALTED' AND halt_reason IS NOT NULL))
);
CREATE TABLE ingestion_batches (
 operator_id text NOT NULL, stream_id text NOT NULL, revision bigint NOT NULL CHECK(revision>0),
 payload_digest text NOT NULL CHECK(payload_digest ~ '^sha256:[0-9a-f]{64}$'),
 payload jsonb NOT NULL CHECK(jsonb_typeof(payload)='object'),
 payload_bytes bigint NOT NULL CHECK(payload_bytes BETWEEN 1 AND 2097152),
 checkpoint jsonb NOT NULL CHECK(jsonb_typeof(checkpoint)='object'),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(operator_id,stream_id,revision),
 FOREIGN KEY(operator_id,stream_id) REFERENCES ingestion_streams(operator_id,stream_id)
);
CREATE TRIGGER append_only_ingestion_batches BEFORE UPDATE OR DELETE ON ingestion_batches
 FOR EACH ROW EXECUTE FUNCTION reject_audit_mutation();
CREATE FUNCTION protect_ingestion_cursor() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'ingestion history cannot be deleted'; END IF;
 IF (NEW.operator_id,NEW.stream_id,NEW.binding,NEW.initial_checkpoint,NEW.created_at)
 IS DISTINCT FROM (OLD.operator_id,OLD.stream_id,OLD.binding,OLD.initial_checkpoint,OLD.created_at)
 THEN RAISE EXCEPTION 'ingestion identity is immutable'; END IF;
 IF OLD.state='HALTED' OR NEW.revision<>OLD.revision+1
 THEN RAISE EXCEPTION 'halted or stale ingestion cursor'; END IF;
 IF NEW.state='HALTED' THEN
  IF NEW.checkpoint IS DISTINCT FROM OLD.checkpoint OR NEW.retained_bytes<>OLD.retained_bytes
  THEN RAISE EXCEPTION 'halt cannot advance ingestion'; END IF;
 ELSIF NOT EXISTS(SELECT 1 FROM ingestion_batches b WHERE b.operator_id=NEW.operator_id
  AND b.stream_id=NEW.stream_id AND b.revision=NEW.revision AND b.checkpoint=NEW.checkpoint
  AND NEW.retained_bytes=OLD.retained_bytes+b.payload_bytes)
 THEN RAISE EXCEPTION 'cursor advancement requires saved batch'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER immutable_ingestion_context BEFORE UPDATE OR DELETE ON ingestion_streams
 FOR EACH ROW EXECUTE FUNCTION protect_ingestion_cursor();
