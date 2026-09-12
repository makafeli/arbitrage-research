-- An attempt is recorded before capture I/O. Missing terminal evidence remains unknown.
-- This journal is independent of financial research_attempts and their drain counters.
CREATE TABLE collection_attempts (
 attempt_id text PRIMARY KEY, operator_id text NOT NULL, session_id text NOT NULL,
 network_id text NOT NULL CHECK(network_id IN ('base-mainnet','solana-mainnet')),
 configuration_digest text NOT NULL, experiment_id text NOT NULL,
 generation bigint NOT NULL CHECK(generation>=0), worker_epoch bigint NOT NULL CHECK(worker_epoch>0), worker_id text NOT NULL,
 purpose text NOT NULL CHECK(purpose IN ('READINESS','RESEARCH')), start_digest text NOT NULL,
 started_at timestamptz NOT NULL DEFAULT clock_timestamp(), finished_at timestamptz,
 outcome text NOT NULL DEFAULT 'IN_PROGRESS' CHECK(outcome IN ('IN_PROGRESS','READINESS_COMPLETED','DECISIONS_RECORDED','ACQUISITION_FAILED','EVALUATION_FAILED','DEADLINE_EXCEEDED','SUPPRESSED','WORKER_CANCELLED')),
 reason text CHECK(reason IN ('PROVIDER_UNAVAILABLE','INPUT_VALIDATION_FAILED','CAPTURE_STORAGE_UNAVAILABLE','RESOURCE_LIMIT','ACQUISITION_UNAVAILABLE','ACQUISITION_DEADLINE','EVALUATION_REJECTED','EVALUATION_DEADLINE','GENERATION_FENCED','WORKER_SHUTDOWN','TASK_FAILED')),
 captured_pools integer NOT NULL DEFAULT 0 CHECK(captured_pools BETWEEN 0 AND 8),
 decision_rows bigint NOT NULL DEFAULT 0 CHECK(decision_rows BETWEEN 0 AND 64),
 elapsed_ms bigint CHECK(elapsed_ms BETWEEN 0 AND 86400000),
 finish_digest text, decision_observation_ids jsonb NOT NULL DEFAULT '[]',
 FOREIGN KEY(operator_id,session_id) REFERENCES research_sessions(operator_id,session_id),
 FOREIGN KEY(operator_id,configuration_digest) REFERENCES configuration_snapshots(operator_id,configuration_digest),
 CHECK((outcome='IN_PROGRESS' AND finished_at IS NULL AND finish_digest IS NULL AND elapsed_ms IS NULL AND reason IS NULL AND captured_pools=0 AND decision_rows=0)
    OR (outcome<>'IN_PROGRESS' AND finished_at IS NOT NULL AND finish_digest IS NOT NULL AND elapsed_ms IS NOT NULL)),
 CHECK(jsonb_typeof(decision_observation_ids)='array' AND jsonb_array_length(decision_observation_ids)=decision_rows),
 CHECK((outcome='DECISIONS_RECORDED' AND purpose='RESEARCH' AND decision_rows>0 AND reason IS NULL)
    OR (outcome<>'DECISIONS_RECORDED' AND decision_rows=0)),
 CHECK(outcome<>'READINESS_COMPLETED' OR (purpose='READINESS' AND reason IS NULL)),
 CHECK((outcome IN ('IN_PROGRESS','READINESS_COMPLETED','DECISIONS_RECORDED') AND reason IS NULL)
    OR (outcome='ACQUISITION_FAILED' AND reason IN ('PROVIDER_UNAVAILABLE','INPUT_VALIDATION_FAILED','CAPTURE_STORAGE_UNAVAILABLE','RESOURCE_LIMIT','ACQUISITION_UNAVAILABLE','TASK_FAILED'))
    OR (outcome='EVALUATION_FAILED' AND purpose='RESEARCH' AND reason IN ('EVALUATION_REJECTED','RESOURCE_LIMIT','TASK_FAILED'))
    OR (outcome='DEADLINE_EXCEEDED' AND (reason='ACQUISITION_DEADLINE' OR (purpose='RESEARCH' AND reason='EVALUATION_DEADLINE')))
    OR (outcome='SUPPRESSED' AND reason='GENERATION_FENCED')
    OR (outcome='WORKER_CANCELLED' AND reason='WORKER_SHUTDOWN')),
 CHECK(outcome IN ('IN_PROGRESS','READINESS_COMPLETED','DECISIONS_RECORDED') OR reason IS NOT NULL)
);
CREATE INDEX collection_attempt_page ON collection_attempts(operator_id,session_id,attempt_id);
CREATE FUNCTION protect_collection_attempt() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'collection history cannot be deleted'; END IF;
 IF (NEW.attempt_id,NEW.operator_id,NEW.session_id,NEW.network_id,NEW.configuration_digest,NEW.experiment_id,NEW.generation,NEW.worker_epoch,NEW.worker_id,NEW.purpose,NEW.start_digest,NEW.started_at)
 IS DISTINCT FROM
 (OLD.attempt_id,OLD.operator_id,OLD.session_id,OLD.network_id,OLD.configuration_digest,OLD.experiment_id,OLD.generation,OLD.worker_epoch,OLD.worker_id,OLD.purpose,OLD.start_digest,OLD.started_at) THEN
 RAISE EXCEPTION 'collection attempt identity is immutable';
 END IF;
 IF OLD.outcome<>'IN_PROGRESS' THEN RAISE EXCEPTION 'collection terminal evidence is immutable'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER immutable_collection_attempt BEFORE UPDATE OR DELETE ON collection_attempts FOR EACH ROW EXECUTE FUNCTION protect_collection_attempt();

-- A durable decision can count toward one collection batch only. Legacy decisions may
-- remain unassociated; the collection denominator does not imply historical completeness.
CREATE TABLE collection_decision_links (
 attempt_id text NOT NULL REFERENCES collection_attempts(attempt_id),
 trace_id text NOT NULL UNIQUE REFERENCES decision_traces(trace_id),
 PRIMARY KEY(attempt_id,trace_id)
);
CREATE TRIGGER append_only_collection_decision_links BEFORE UPDATE OR DELETE ON collection_decision_links FOR EACH ROW EXECUTE FUNCTION reject_audit_mutation();
