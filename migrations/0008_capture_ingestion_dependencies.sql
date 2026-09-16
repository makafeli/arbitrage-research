-- Explicit capture-to-stream dependencies. No inference from similar timestamps,
-- pool names or a currently healthy stream. Historical payloads remain immutable.
CREATE TABLE capture_ingestion_dependencies (
 operator_id text NOT NULL,
 session_id text NOT NULL,
 capture_id text NOT NULL,
 manifest_digest text NOT NULL CHECK(manifest_digest ~ '^sha256:[0-9a-f]{64}$'),
 stream_id text NOT NULL,
 snapshot_revision bigint NOT NULL CHECK(snapshot_revision BETWEEN 1 AND 4096),
 snapshot_digest text NOT NULL CHECK(snapshot_digest ~ '^sha256:[0-9a-f]{64}$'),
 checkpoint jsonb NOT NULL CHECK(jsonb_typeof(checkpoint)='object'),
 policy_version text NOT NULL DEFAULT 'base-capture-continuity-v1'
  CHECK(policy_version='base-capture-continuity-v1'),
 linked_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(session_id,capture_id),
 FOREIGN KEY(operator_id,session_id) REFERENCES research_sessions(operator_id,session_id),
 FOREIGN KEY(session_id,capture_id) REFERENCES capture_admissions(session_id,capture_id),
 FOREIGN KEY(operator_id,stream_id,snapshot_revision)
  REFERENCES ingestion_batches(operator_id,stream_id,revision)
);
CREATE INDEX capture_ingestion_source ON capture_ingestion_dependencies
 (operator_id,stream_id,snapshot_revision);

CREATE FUNCTION validate_capture_ingestion_dependency() RETURNS trigger
 LANGUAGE plpgsql SET lock_timeout='2s' AS $$
DECLARE
 network text;
 admitted_digest text;
 source_payload jsonb;
BEGIN
 -- Use the same session lock as ordinary worker publication. In particular,
 -- first association and an untracked publication cannot race past each other.
 SELECT network_id INTO network FROM research_sessions
  WHERE operator_id=NEW.operator_id AND session_id=NEW.session_id FOR UPDATE;
 IF NOT FOUND THEN
  RAISE EXCEPTION USING ERRCODE='P0002',MESSAGE='capture session not found';
 END IF;
 IF network<>'base-mainnet' THEN
  RAISE EXCEPTION USING ERRCODE='22023',MESSAGE='unsupported continuity network';
 END IF;
 SELECT manifest_digest INTO admitted_digest FROM capture_admissions
  WHERE session_id=NEW.session_id AND capture_id=NEW.capture_id;
 IF NOT FOUND OR admitted_digest IS DISTINCT FROM NEW.manifest_digest THEN
  RAISE EXCEPTION USING ERRCODE='22023',MESSAGE='capture admission reference mismatch';
 END IF;
 -- Never retroactively promote an old untracked decision by attaching evidence.
 IF EXISTS (
  SELECT 1 FROM decision_traces d
  WHERE d.session_id=NEW.session_id AND EXISTS (
   SELECT 1 FROM jsonb_array_elements(CASE
    WHEN jsonb_typeof(d.payload->'capture_refs')='array' THEN d.payload->'capture_refs'
    ELSE '[]'::jsonb END) r
   WHERE r->>'capture_id'=NEW.capture_id
  )
 ) THEN
  RAISE EXCEPTION USING ERRCODE='55000',MESSAGE='capture already has decision history';
 END IF;
 source_payload:=require_current_ingestion_snapshot(
  NEW.operator_id,NEW.stream_id,NEW.snapshot_revision,NEW.snapshot_digest);
 IF NEW.checkpoint IS DISTINCT FROM source_payload->'through' THEN
  RAISE EXCEPTION USING ERRCODE='22023',MESSAGE='capture checkpoint mismatch';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER valid_capture_ingestion_dependency BEFORE INSERT ON capture_ingestion_dependencies
 FOR EACH ROW EXECUTE FUNCTION validate_capture_ingestion_dependency();
CREATE TRIGGER immutable_capture_ingestion_dependency BEFORE UPDATE OR DELETE ON capture_ingestion_dependencies
 FOR EACH ROW EXECUTE FUNCTION reject_audit_mutation();

-- Internal invoker-rights gate. Callers must keep the SHORT transaction open
-- until publication finishes. This checks continuity, not freshness or profit.
CREATE FUNCTION require_bound_decision_captures(
 p_operator text,p_session text,p_payload jsonb
) RETURNS void LANGUAGE plpgsql SET lock_timeout='2s' AS $$
DECLARE
 network text;
 refs jsonb;
 ref_count integer;
 distinct_count integer;
 matched_count integer;
 dependency record;
BEGIN
 SELECT network_id INTO network FROM research_sessions
  WHERE operator_id=p_operator AND session_id=p_session FOR UPDATE;
 IF NOT FOUND THEN
  RAISE EXCEPTION USING ERRCODE='P0002',MESSAGE='decision session not found';
 END IF;
 IF network<>'base-mainnet'
  OR p_payload->>'network_id' IS DISTINCT FROM network
  OR p_payload->>'session_id' IS DISTINCT FROM p_session THEN
  RAISE EXCEPTION USING ERRCODE='22023',MESSAGE='decision continuity scope mismatch';
 END IF;
 refs:=p_payload->'capture_refs';
 IF jsonb_typeof(refs) IS DISTINCT FROM 'array' THEN
  RAISE EXCEPTION USING ERRCODE='22023',MESSAGE='invalid capture references';
 END IF;
 ref_count:=jsonb_array_length(refs);
 IF ref_count NOT BETWEEN 1 AND 8 THEN
  RAISE EXCEPTION USING ERRCODE='22023',MESSAGE='invalid capture reference count';
 END IF;
 SELECT count(DISTINCT r->>'capture_id') INTO distinct_count
  FROM jsonb_array_elements(refs) r;
 IF distinct_count<>ref_count THEN
  RAISE EXCEPTION USING ERRCODE='22023',MESSAGE='duplicate or missing capture identity';
 END IF;
 SELECT count(*) INTO matched_count FROM jsonb_array_elements(refs) r
  JOIN capture_ingestion_dependencies d
   ON d.operator_id=p_operator AND d.session_id=p_session
   AND d.capture_id=r->>'capture_id' AND d.manifest_digest=r->>'manifest_digest'
   AND d.manifest_digest=r->>'snapshot_id'
  JOIN capture_admissions a ON a.session_id=d.session_id AND a.capture_id=d.capture_id
   AND a.generation::text=p_payload->>'generation';
 IF matched_count<>ref_count THEN
  RAISE EXCEPTION USING ERRCODE='55000',MESSAGE='decision capture continuity untracked';
 END IF;
 -- A deterministic source order avoids opposite-route stream-lock inversions.
 FOR dependency IN
  SELECT DISTINCT d.stream_id,d.snapshot_revision,d.snapshot_digest
  FROM jsonb_array_elements(refs) r
  JOIN capture_ingestion_dependencies d
   ON d.operator_id=p_operator AND d.session_id=p_session AND d.capture_id=r->>'capture_id'
  ORDER BY d.stream_id,d.snapshot_revision,d.snapshot_digest
 LOOP
  PERFORM require_current_ingestion_snapshot(p_operator,dependency.stream_id,
   dependency.snapshot_revision,dependency.snapshot_digest);
 END LOOP;
END $$;

CREATE FUNCTION guard_bound_decision_publication() RETURNS trigger
 LANGUAGE plpgsql SET lock_timeout='2s' AS $$
BEGIN
 -- Existing worker insertion already holds this row; direct insertion must do
 -- so as well. Negative diagnostics remain recordable after a source failure.
 PERFORM 1 FROM research_sessions
  WHERE operator_id=NEW.operator_id AND session_id=NEW.session_id FOR UPDATE;
 IF NEW.result_status='QUOTED' AND EXISTS (
  SELECT 1 FROM capture_ingestion_dependencies
  WHERE operator_id=NEW.operator_id AND session_id=NEW.session_id
 ) THEN
  PERFORM require_bound_decision_captures(NEW.operator_id,NEW.session_id,NEW.payload);
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER enforce_bound_decision_continuity BEFORE INSERT ON decision_traces
 FOR EACH ROW EXECUTE FUNCTION guard_bound_decision_publication();

CREATE VIEW decision_ingestion_validity AS
 SELECT d.operator_id,d.session_id,d.trace_id,d.observation_id,
  'base-capture-continuity-v1'::text AS policy_version,
  refs.capture_count,refs.bound_count,
  CASE
   WHEN d.payload->>'network_id' IS DISTINCT FROM 'base-mainnet' THEN 'UNVERIFIABLE'
   WHEN refs.invalid_count>0 THEN 'INVALIDATED'
   WHEN refs.capture_count=0 OR refs.bound_count<>refs.capture_count THEN 'UNTRACKED'
   WHEN refs.distinct_count<>refs.capture_count THEN 'UNVERIFIABLE'
   WHEN refs.current_count=refs.capture_count THEN 'NO_KNOWN_INVALIDATION'
   ELSE 'UNVERIFIABLE'
  END AS continuity_status
 FROM decision_traces d
 CROSS JOIN LATERAL (
  SELECT count(*) AS capture_count,count(DISTINCT r->>'capture_id') AS distinct_count,
   count(c.capture_id) AS bound_count,
   count(*) FILTER(WHERE v.continuity_status='INVALIDATED') AS invalid_count,
   count(*) FILTER(WHERE v.continuity_status='NO_KNOWN_INVALIDATION'
    AND v.snapshot_digest=c.snapshot_digest
    AND v.snapshot_checkpoint=c.checkpoint) AS current_count
  FROM jsonb_array_elements(CASE WHEN jsonb_typeof(d.payload->'capture_refs')='array'
   THEN d.payload->'capture_refs' ELSE '[]'::jsonb END) r
  LEFT JOIN capture_ingestion_dependencies c
   ON c.operator_id=d.operator_id AND c.session_id=d.session_id
   AND c.capture_id=r->>'capture_id' AND c.manifest_digest=r->>'manifest_digest'
   AND c.manifest_digest=r->>'snapshot_id'
  LEFT JOIN ingestion_snapshot_validity v ON v.operator_id=c.operator_id
   AND v.stream_id=c.stream_id AND v.snapshot_revision=c.snapshot_revision
 ) refs;

CREATE FUNCTION require_current_decision_ingestion(
 p_operator text,p_session text,p_observation text
) RETURNS jsonb LANGUAGE plpgsql SET lock_timeout='2s' AS $$
DECLARE
 original jsonb;
BEGIN
 SELECT payload INTO original FROM decision_traces
  WHERE operator_id=p_operator AND session_id=p_session AND observation_id=p_observation;
 IF NOT FOUND THEN
  RAISE EXCEPTION USING ERRCODE='P0002',MESSAGE='decision not found';
 END IF;
 PERFORM require_bound_decision_captures(p_operator,p_session,original);
 RETURN original;
END $$;
