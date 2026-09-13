-- Immutable hypothetical cost research, bound to its stored source decision.
-- No balance, lifecycle, simulation or execution state is changed by this table.
ALTER TABLE decision_traces ADD CONSTRAINT decision_trace_scoped_identity UNIQUE(operator_id,session_id,trace_id);
CREATE TABLE cost_assessments (
 record_id text PRIMARY KEY,
 operator_id text NOT NULL,
 session_id text NOT NULL,
 source_trace_id text NOT NULL,
 observation_id text NOT NULL,
 configuration_digest text NOT NULL,
 experiment_id text NOT NULL,
 network_id text NOT NULL CHECK(network_id IN ('base-mainnet','solana-mainnet')),
 idempotency_key text NOT NULL,
 request_digest text NOT NULL,
 payload_digest text NOT NULL,
 payload jsonb NOT NULL,
 recorded_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 UNIQUE(operator_id,session_id,idempotency_key),
 FOREIGN KEY(operator_id,session_id) REFERENCES research_sessions(operator_id,session_id),
 FOREIGN KEY(operator_id,session_id,source_trace_id) REFERENCES decision_traces(operator_id,session_id,trace_id),
 FOREIGN KEY(operator_id,configuration_digest) REFERENCES configuration_snapshots(operator_id,configuration_digest)
);
CREATE INDEX cost_assessment_page ON cost_assessments(operator_id,session_id,record_id);
CREATE TRIGGER append_only_cost_assessments BEFORE UPDATE OR DELETE ON cost_assessments FOR EACH ROW EXECUTE FUNCTION reject_audit_mutation();
