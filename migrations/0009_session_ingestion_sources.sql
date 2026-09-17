-- Persist the explicit research-worker source requirement before its first quote.
-- Existing sessions are not retroactively associated with unrelated event streams.
CREATE TABLE session_ingestion_sources (
 operator_id text NOT NULL,
 session_id text NOT NULL,
 stream_id text NOT NULL,
 policy_version text NOT NULL DEFAULT 'base-capture-continuity-v1'
  CHECK(policy_version='base-capture-continuity-v1'),
 configured_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(operator_id,session_id),
 FOREIGN KEY(operator_id,session_id) REFERENCES research_sessions(operator_id,session_id),
 FOREIGN KEY(operator_id,stream_id) REFERENCES ingestion_streams(operator_id,stream_id)
);

CREATE FUNCTION validate_session_ingestion_source() RETURNS trigger
 LANGUAGE plpgsql SET lock_timeout='2s' AS $$
DECLARE
 network text;
 source_state text;
 source_network text;
BEGIN
 SELECT network_id INTO network FROM research_sessions
  WHERE operator_id=NEW.operator_id AND session_id=NEW.session_id FOR UPDATE;
 IF NOT FOUND OR network<>'base-mainnet' THEN
  RAISE EXCEPTION USING ERRCODE='22023',MESSAGE='invalid capture source session';
 END IF;
 IF EXISTS(SELECT 1 FROM decision_traces
  WHERE operator_id=NEW.operator_id AND session_id=NEW.session_id) THEN
  RAISE EXCEPTION USING ERRCODE='55000',MESSAGE='capture source cannot promote existing decisions';
 END IF;
 IF EXISTS(SELECT 1 FROM capture_ingestion_dependencies
  WHERE operator_id=NEW.operator_id AND session_id=NEW.session_id AND stream_id<>NEW.stream_id) THEN
  RAISE EXCEPTION USING ERRCODE='55000',MESSAGE='capture source conflicts with existing dependencies';
 END IF;
 SELECT state,binding->>'network_id' INTO source_state,source_network FROM ingestion_streams
  WHERE operator_id=NEW.operator_id AND stream_id=NEW.stream_id FOR SHARE;
 IF NOT FOUND OR source_state<>'ACTIVE' OR source_network<>'base-mainnet' THEN
  RAISE EXCEPTION USING ERRCODE='55000',MESSAGE='capture source unavailable';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER valid_session_ingestion_source BEFORE INSERT ON session_ingestion_sources
 FOR EACH ROW EXECUTE FUNCTION validate_session_ingestion_source();
CREATE TRIGGER immutable_session_ingestion_source BEFORE UPDATE OR DELETE ON session_ingestion_sources
 FOR EACH ROW EXECUTE FUNCTION reject_audit_mutation();

CREATE FUNCTION require_configured_capture_source() RETURNS trigger
 LANGUAGE plpgsql SET lock_timeout='2s' AS $$
DECLARE
 required_stream text;
BEGIN
 PERFORM 1 FROM research_sessions
  WHERE operator_id=NEW.operator_id AND session_id=NEW.session_id FOR UPDATE;
 SELECT stream_id INTO required_stream FROM session_ingestion_sources
  WHERE operator_id=NEW.operator_id AND session_id=NEW.session_id;
 IF FOUND AND required_stream<>NEW.stream_id THEN
  RAISE EXCEPTION USING ERRCODE='55000',MESSAGE='capture dependency uses another configured source';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER enforce_configured_capture_source BEFORE INSERT ON capture_ingestion_dependencies
 FOR EACH ROW EXECUTE FUNCTION require_configured_capture_source();

CREATE OR REPLACE FUNCTION guard_bound_decision_publication() RETURNS trigger
 LANGUAGE plpgsql SET lock_timeout='2s' AS $$
BEGIN
 PERFORM 1 FROM research_sessions
  WHERE operator_id=NEW.operator_id AND session_id=NEW.session_id FOR UPDATE;
 IF NEW.result_status='QUOTED' AND (
  EXISTS(SELECT 1 FROM session_ingestion_sources
   WHERE operator_id=NEW.operator_id AND session_id=NEW.session_id)
  OR EXISTS(SELECT 1 FROM capture_ingestion_dependencies
   WHERE operator_id=NEW.operator_id AND session_id=NEW.session_id)
 ) THEN
  PERFORM require_bound_decision_captures(NEW.operator_id,NEW.session_id,NEW.payload);
 END IF;
 RETURN NEW;
END $$;
