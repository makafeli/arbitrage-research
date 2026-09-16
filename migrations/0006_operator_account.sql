-- One private operator account; existing research ownership remains "operator".
CREATE TABLE operator_account (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    email text,
    password_salt bytea,
    password_hash bytea,
    auth_version bigint NOT NULL DEFAULT 0 CHECK (auth_version >= 0),
    changed_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    CHECK ((email IS NULL AND password_salt IS NULL AND password_hash IS NULL AND auth_version = 0)
        OR (email IS NOT NULL AND password_salt IS NOT NULL AND password_hash IS NOT NULL AND length(email) BETWEEN 3 AND 254 AND email = lower(email)
            AND octet_length(password_salt) = 32 AND octet_length(password_hash) = 32 AND auth_version > 0))
);
INSERT INTO operator_account(singleton) VALUES(true);
CREATE TABLE operator_account_invitation (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    token_hash bytea NOT NULL CHECK (octet_length(token_hash) = 32),
    email text CHECK (length(email) BETWEEN 3 AND 254 AND email = lower(email)),
    account_version bigint NOT NULL CHECK (account_version >= 0),
    expires_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
