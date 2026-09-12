-- Append-only decisions and exact bounded virtual accounting; no execution permissions.
CREATE TABLE decision_traces (
 trace_id text PRIMARY KEY, operator_id text NOT NULL, session_id text NOT NULL,
 observation_id text NOT NULL, payload_digest text NOT NULL, configuration_digest text NOT NULL,
 generation bigint NOT NULL CHECK(generation>=0), observed_at_unix_ms bigint NOT NULL CHECK(observed_at_unix_ms>0),
 recorded_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 result_status text NOT NULL CHECK(result_status IN ('QUOTED','REJECTED','NO_ROUTE','DATA_UNAVAILABLE')),
 grouping_version text NOT NULL, grouping_key text NOT NULL, window_start_ms bigint NOT NULL CHECK(window_start_ms>=0),
 payload jsonb NOT NULL, UNIQUE(session_id,observation_id),
 FOREIGN KEY(operator_id,session_id) REFERENCES research_sessions(operator_id,session_id),
 FOREIGN KEY(operator_id,configuration_digest) REFERENCES configuration_snapshots(operator_id,configuration_digest)
);
CREATE INDEX decision_trace_page ON decision_traces(operator_id,session_id,trace_id);
CREATE INDEX decision_trace_groups ON decision_traces(session_id,grouping_key);
CREATE TRIGGER append_only_decision_traces BEFORE UPDATE OR DELETE ON decision_traces FOR EACH ROW EXECUTE FUNCTION reject_audit_mutation();

CREATE TABLE paper_runs (
 run_id text PRIMARY KEY, operator_id text NOT NULL, session_id text NOT NULL, configuration_digest text NOT NULL,
 network_id text NOT NULL CHECK(network_id IN ('base-mainnet','solana-mainnet')), initial_balances jsonb NOT NULL,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), revision bigint NOT NULL DEFAULT 0 CHECK(revision BETWEEN 0 AND 4999),
 journal_bytes bigint NOT NULL CHECK(journal_bytes BETWEEN 0 AND 16777216),
 UNIQUE(operator_id,run_id),
 FOREIGN KEY(operator_id,session_id) REFERENCES research_sessions(operator_id,session_id),
 FOREIGN KEY(operator_id,configuration_digest) REFERENCES configuration_snapshots(operator_id,configuration_digest)
);
CREATE TABLE paper_run_creation_keys (
 operator_id text NOT NULL, session_id text NOT NULL, idempotency_key text NOT NULL, payload_digest text NOT NULL,
 run_id text NOT NULL REFERENCES paper_runs(run_id), PRIMARY KEY(operator_id,session_id,idempotency_key)
);
CREATE TABLE paper_journal (
 event_id text PRIMARY KEY, run_id text NOT NULL REFERENCES paper_runs(run_id), sequence bigint NOT NULL CHECK(sequence BETWEEN 0 AND 4999),
 command_id text NOT NULL, payload_digest text NOT NULL, payload jsonb NOT NULL,
 recorded_at timestamptz NOT NULL DEFAULT clock_timestamp(), UNIQUE(run_id,sequence), UNIQUE(run_id,command_id)
);
CREATE INDEX paper_run_page ON paper_runs(operator_id,session_id,run_id);
CREATE TRIGGER append_only_paper_journal BEFORE UPDATE OR DELETE ON paper_journal FOR EACH ROW EXECUTE FUNCTION reject_audit_mutation();
CREATE TRIGGER append_only_paper_run_creation_keys BEFORE UPDATE OR DELETE ON paper_run_creation_keys FOR EACH ROW EXECUTE FUNCTION reject_audit_mutation();
CREATE FUNCTION protect_paper_run_identity() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'paper run history cannot be deleted'; END IF;
 IF (NEW.run_id,NEW.operator_id,NEW.session_id,NEW.configuration_digest,NEW.network_id,NEW.initial_balances,NEW.created_at)
 IS DISTINCT FROM
 (OLD.run_id,OLD.operator_id,OLD.session_id,OLD.configuration_digest,OLD.network_id,OLD.initial_balances,OLD.created_at) THEN
 RAISE EXCEPTION 'paper initial balances and identity are immutable';
 END IF;
 IF NEW.revision<OLD.revision OR NEW.journal_bytes<OLD.journal_bytes THEN RAISE EXCEPTION 'journal cursor cannot move backward'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER immutable_paper_run BEFORE UPDATE OR DELETE ON paper_runs FOR EACH ROW EXECUTE FUNCTION protect_paper_run_identity();
