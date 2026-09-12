-- Forward-only control journal. See README.md before rollback or restore.
CREATE TABLE configuration_snapshots (
    operator_id text NOT NULL,
    configuration_digest text NOT NULL,
    snapshot jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (operator_id, configuration_digest)
);
CREATE TABLE research_sessions (
    session_id text PRIMARY KEY,
    operator_id text NOT NULL,
    network_id text NOT NULL CHECK (network_id IN ('base-mainnet', 'solana-mainnet')),
    mode text NOT NULL CHECK (mode IN ('OBSERVE', 'PAPER', 'REPLAY')),
    configuration_digest text NOT NULL,
    experiment_id text NOT NULL,
    strategy_ids jsonb NOT NULL CHECK (jsonb_typeof(strategy_ids) = 'array'),
    observed_state text NOT NULL CHECK (observed_state IN ('RECOVERING','STOPPED','RUNNING','PAUSING','PAUSED','DRAINING','FAULTED')),
    desired_revision bigint NOT NULL DEFAULT 0 CHECK (desired_revision >= 0),
    applied_revision bigint NOT NULL DEFAULT 0 CHECK (applied_revision >= 0 AND applied_revision <= desired_revision),
    outstanding_attempts bigint NOT NULL DEFAULT 0 CHECK (outstanding_attempts BETWEEN 0 AND 4294967295),
    generation bigint NOT NULL DEFAULT 0 CHECK (generation >= 0),
    local_fence boolean NOT NULL DEFAULT true,
    lifecycle jsonb NOT NULL,
    worker_id text,
    worker_epoch bigint NOT NULL DEFAULT 0 CHECK (worker_epoch >= 0),
    lease_until timestamptz,
    last_heartbeat_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (operator_id, session_id),
    FOREIGN KEY (operator_id, configuration_digest) REFERENCES configuration_snapshots(operator_id, configuration_digest),
    CHECK (local_fence OR observed_state = 'RUNNING')
);
CREATE INDEX research_sessions_operator_page ON research_sessions(operator_id,session_id);
CREATE TABLE session_creation_keys (
    operator_id text NOT NULL,
    idempotency_key text NOT NULL,
    payload_digest text NOT NULL,
    session_id text NOT NULL,
    PRIMARY KEY (operator_id,idempotency_key),
    FOREIGN KEY (operator_id,session_id) REFERENCES research_sessions(operator_id,session_id)
);
CREATE TABLE control_commands (
    command_id text PRIMARY KEY,
    operator_id text NOT NULL,
    session_id text NOT NULL,
    idempotency_key text NOT NULL,
    payload_digest text NOT NULL,
    action text NOT NULL CHECK (action IN ('START','PAUSE','RESUME','STOP')),
    revision bigint NOT NULL CHECK (revision > 0),
    status text NOT NULL CHECK (status IN ('PENDING','APPLIED','REJECTED','SUPERSEDED')),
    reason text,
    accepted_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    applied_at timestamptz,
    outstanding_attempts bigint NOT NULL CHECK (outstanding_attempts BETWEEN 0 AND 4294967295),
    fence_effective boolean NOT NULL DEFAULT false,
    UNIQUE (operator_id,session_id,idempotency_key),
    UNIQUE (session_id,revision),
    FOREIGN KEY (operator_id,session_id) REFERENCES research_sessions(operator_id,session_id),
    CHECK ((status = 'APPLIED') = (applied_at IS NOT NULL)),
    CHECK (status <> 'APPLIED' OR action NOT IN ('PAUSE','STOP') OR fence_effective)
);
CREATE UNIQUE INDEX control_commands_one_pending ON control_commands(session_id) WHERE status = 'PENDING';
CREATE TABLE control_audit_events (
    audit_id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    session_id text NOT NULL REFERENCES research_sessions(session_id),
    operator_id text NOT NULL,
    event_kind text NOT NULL,
    command_id text REFERENCES control_commands(command_id),
    detail jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
-- Application and admin accidents cannot silently change identity/config or erase audit.
CREATE FUNCTION protect_session_identity() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF (NEW.operator_id,NEW.network_id,NEW.mode,NEW.configuration_digest,NEW.experiment_id,NEW.strategy_ids)
       IS DISTINCT FROM
       (OLD.operator_id,OLD.network_id,OLD.mode,OLD.configuration_digest,OLD.experiment_id,OLD.strategy_ids) THEN
        RAISE EXCEPTION 'session identity and configuration are immutable';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER immutable_session_identity BEFORE UPDATE ON research_sessions
    FOR EACH ROW EXECUTE FUNCTION protect_session_identity();
CREATE FUNCTION reject_audit_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'control audit is append-only'; END $$;
CREATE TRIGGER append_only_control_audit BEFORE UPDATE OR DELETE ON control_audit_events
    FOR EACH ROW EXECUTE FUNCTION reject_audit_mutation();
CREATE TRIGGER immutable_configuration_snapshots BEFORE UPDATE OR DELETE ON configuration_snapshots
    FOR EACH ROW EXECUTE FUNCTION reject_audit_mutation();
-- These identities represent research work only, not signed transactions or a ledger.
CREATE TABLE research_attempts (
    attempt_id text PRIMARY KEY,
    session_id text NOT NULL REFERENCES research_sessions(session_id),
    generation bigint NOT NULL CHECK (generation >= 0),
    admitted_worker_epoch bigint NOT NULL CHECK (admitted_worker_epoch > 0),
    status text NOT NULL CHECK (status IN ('OUTSTANDING','RESOLVED')),
    resolution_evidence text,
    admitted_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    resolved_at timestamptz,
    CHECK ((status = 'RESOLVED') = (resolved_at IS NOT NULL)),
    CHECK ((status = 'RESOLVED') = (resolution_evidence IS NOT NULL))
);
CREATE INDEX research_attempts_outstanding ON research_attempts(session_id) WHERE status='OUTSTANDING';
CREATE FUNCTION protect_research_attempt() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='DELETE' THEN RAISE EXCEPTION 'research attempt history cannot be deleted'; END IF;
    IF (NEW.attempt_id,NEW.session_id,NEW.generation,NEW.admitted_worker_epoch,NEW.admitted_at)
       IS DISTINCT FROM
       (OLD.attempt_id,OLD.session_id,OLD.generation,OLD.admitted_worker_epoch,OLD.admitted_at) THEN
        RAISE EXCEPTION 'research attempt identity is immutable';
    END IF;
    IF OLD.status='RESOLVED' AND NEW IS DISTINCT FROM OLD THEN
        RAISE EXCEPTION 'research attempt resolution is terminal';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER protect_research_attempt_history BEFORE UPDATE OR DELETE ON research_attempts
    FOR EACH ROW EXECUTE FUNCTION protect_research_attempt();
CREATE TABLE capture_admissions (
    session_id text NOT NULL REFERENCES research_sessions(session_id),
    capture_id text NOT NULL,
    attempt_id text NOT NULL UNIQUE REFERENCES research_attempts(attempt_id),
    manifest_digest text NOT NULL,
    artifact_path text NOT NULL,
    generation bigint NOT NULL CHECK (generation >= 0),
    admitted_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY(session_id,capture_id)
);
CREATE TRIGGER append_only_capture_admissions BEFORE UPDATE OR DELETE ON capture_admissions
    FOR EACH ROW EXECUTE FUNCTION reject_audit_mutation();
