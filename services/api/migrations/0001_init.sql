-- PQ-CAS / KEM DEMO / CAVP schema.
-- Status vocabularies are Postgres enums so bad values cannot reach the matrix.

CREATE TYPE task_status     AS ENUM ('pending', 'running', 'finished', 'failed', 'cancelled');
CREATE TYPE exec_status     AS ENUM ('queued', 'running', 'finished', 'error');
CREATE TYPE result_status   AS ENUM ('pending', 'passed', 'failed', 'unsupported', 'disabled');
CREATE TYPE report_status   AS ENUM ('missing', 'partial', 'complete');
CREATE TYPE kem_run_mode    AS ENUM ('library_compat', 'group_compat', 'priority');
CREATE TYPE cavp_capability AS ENUM ('key_gen', 'sig_gen', 'sig_ver');

-- Lab targets shown on the Environment page and the KEM DEMO endpoint dropdown.
CREATE TABLE endpoints (
    id        uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    slug      text NOT NULL UNIQUE,
    label     text NOT NULL,
    label_zh  text NOT NULL,
    host      text NOT NULL,
    port      integer NOT NULL CHECK (port BETWEEN 1 AND 65535),
    notes     text,
    enabled   boolean NOT NULL DEFAULT true
);

-- Seeded catalog driving every checkbox grid and matrix axis. `enabled = false` renders
-- greyed out (the 未啟用 / not-offered state in the UI).
CREATE TABLE algorithms (
    id           serial PRIMARY KEY,
    kind         text NOT NULL CHECK (kind IN ('kem_group', 'sig_alg')),
    name         text NOT NULL,
    display_name text NOT NULL,
    family       text,
    enabled      boolean NOT NULL DEFAULT true,
    sort_order   integer NOT NULL DEFAULT 0,
    UNIQUE (kind, name)
);

CREATE TABLE tests (
    id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    host           text NOT NULL,
    port           integer NOT NULL CHECK (port BETWEEN 1 AND 65535),
    http_path      text NOT NULL DEFAULT '/',
    method         text NOT NULL DEFAULT 'GET',
    tool           text NOT NULL,
    plugin         text NOT NULL,
    status         task_status NOT NULL DEFAULT 'pending',
    group_count    integer NOT NULL DEFAULT 0,
    sig_alg_count  integer NOT NULL DEFAULT 0,
    subtask_count  integer NOT NULL DEFAULT 0,
    created_at     timestamptz NOT NULL DEFAULT now(),
    started_at     timestamptz,
    finished_at    timestamptz
);

CREATE INDEX tests_created_at_idx ON tests (created_at DESC);

CREATE TABLE subtasks (
    id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    test_id        uuid NOT NULL REFERENCES tests (id) ON DELETE CASCADE,
    ordinal        integer NOT NULL,
    group_name     text NOT NULL,
    sig_alg        text NOT NULL,
    exec_status    exec_status NOT NULL DEFAULT 'queued',
    result_status  result_status NOT NULL DEFAULT 'pending',
    report_status  report_status NOT NULL DEFAULT 'missing',
    failed_stage   text,
    error_summary  text,
    started_at     timestamptz,
    finished_at    timestamptz,
    UNIQUE (test_id, group_name, sig_alg)
);

-- Previous/Next in the subtask drawer walks this index.
CREATE UNIQUE INDEX subtasks_test_ordinal_idx ON subtasks (test_id, ordinal);

-- Split out so painting the matrix never drags the heavy blobs along.
CREATE TABLE subtask_reports (
    subtask_id  uuid PRIMARY KEY REFERENCES subtasks (id) ON DELETE CASCADE,
    adapter     text NOT NULL,
    tls_summary jsonb NOT NULL DEFAULT '{}'::jsonb,
    messages    jsonb NOT NULL DEFAULT '[]'::jsonb,
    cert_chain  jsonb NOT NULL DEFAULT '[]'::jsonb,
    http        jsonb,
    diagnostics jsonb NOT NULL DEFAULT '{}'::jsonb,
    raw         jsonb NOT NULL DEFAULT '{}'::jsonb,
    evidence    text,
    duration_ms bigint NOT NULL DEFAULT 0
);

CREATE TABLE kem_runs (
    id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    mode        kem_run_mode NOT NULL,
    endpoint_id uuid NOT NULL REFERENCES endpoints (id) ON DELETE RESTRICT,
    status      task_status NOT NULL DEFAULT 'pending',
    kem_groups  text[] NOT NULL DEFAULT '{}',
    created_at  timestamptz NOT NULL DEFAULT now(),
    finished_at timestamptz
);

CREATE TABLE kem_run_logs (
    run_id  uuid NOT NULL REFERENCES kem_runs (id) ON DELETE CASCADE,
    seq     integer NOT NULL,
    at      timestamptz NOT NULL DEFAULT now(),
    level   text NOT NULL DEFAULT 'info',
    message text NOT NULL,
    PRIMARY KEY (run_id, seq)
);

CREATE TABLE kem_matrix_cells (
    run_id  uuid NOT NULL REFERENCES kem_runs (id) ON DELETE CASCADE,
    kem     text NOT NULL,
    adapter text NOT NULL,
    verdict result_status NOT NULL,
    detail  text,
    PRIMARY KEY (run_id, kem, adapter)
);

-- A fixed question pack: the prompt students see plus the answer key used to grade them.
CREATE TABLE cavp_packs (
    id            uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    slug          text NOT NULL UNIQUE,
    capability    cavp_capability NOT NULL,
    parameter_set text NOT NULL,
    vs_id         bigint NOT NULL,
    prompt        jsonb NOT NULL,
    answer_key    jsonb NOT NULL,
    registration  jsonb NOT NULL DEFAULT '{}'::jsonb
);

CREATE TABLE cavp_sessions (
    id            uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    pack_id       uuid NOT NULL REFERENCES cavp_packs (id) ON DELETE CASCADE,
    capability    cavp_capability NOT NULL,
    parameter_set text NOT NULL,
    vs_id         bigint NOT NULL,
    prompt        jsonb NOT NULL,
    submitted     jsonb,
    validation    jsonb,
    created_at    timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX cavp_sessions_created_at_idx ON cavp_sessions (created_at DESC);
