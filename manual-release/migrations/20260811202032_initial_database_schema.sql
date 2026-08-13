CREATE TABLE projects (
    id UUID PRIMARY KEY,

    name TEXT NOT NULL,
    repository_path TEXT NOT NULL,
    repository_url TEXT,
    default_branch TEXT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT projects_name_not_empty
        CHECK (BTRIM(name) <> ''),

    CONSTRAINT projects_repository_path_not_empty
        CHECK (BTRIM(repository_path) <> ''),

    CONSTRAINT projects_name_unique
        UNIQUE (name),

    CONSTRAINT projects_repository_path_unique
        UNIQUE (repository_path)
);


CREATE TABLE environments (
    id UUID PRIMARY KEY,

    project_id UUID NOT NULL
        REFERENCES projects(id)
        ON DELETE CASCADE,

    name TEXT NOT NULL,

    environment_type TEXT NOT NULL,

    ssh_host TEXT NOT NULL,
    ssh_port INTEGER NOT NULL,
    ssh_user TEXT NOT NULL,

    remote_app_directory TEXT NOT NULL,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT environments_type_valid
        CHECK (
            environment_type IN (
                'STAGING',
                'PRODUCTION'
            )
        ),

    CONSTRAINT environments_port_valid
        CHECK (
            ssh_port > 0
            AND ssh_port <= 65535
        ),

    CONSTRAINT environments_name_not_empty
        CHECK (BTRIM(name) <> ''),

    CONSTRAINT environments_host_not_empty
        CHECK (BTRIM(ssh_host) <> ''),

    CONSTRAINT environments_user_not_empty
        CHECK (BTRIM(ssh_user) <> ''),

    CONSTRAINT environments_remote_directory_not_empty
        CHECK (BTRIM(remote_app_directory) <> ''),

    CONSTRAINT environments_project_name_unique
        UNIQUE (project_id, name),

    CONSTRAINT environments_project_type_unique
        UNIQUE (project_id, environment_type)
);


CREATE TABLE releases (
    id UUID PRIMARY KEY,

    project_id UUID NOT NULL
        REFERENCES projects(id)
        ON DELETE RESTRICT,

    version TEXT NOT NULL,

    git_commit TEXT NOT NULL,
    git_branch TEXT,

    source_dirty BOOLEAN NOT NULL DEFAULT FALSE,

    status TEXT NOT NULL DEFAULT 'CREATED',

    requested_by TEXT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT releases_version_not_empty
        CHECK (BTRIM(version) <> ''),

    CONSTRAINT releases_git_commit_valid
        CHECK (
            git_commit ~ '^[0-9a-f]{40}([0-9a-f]{24})?$'
        ),

    CONSTRAINT releases_status_valid
        CHECK (
            status IN (
                'CREATED',
                'SOURCE_VALIDATED',
                'CI_RUNNING',
                'CI_PASSED',
                'IMAGE_BUILT',
                'IMAGE_TESTED',
                'SCAN_PASSED',
                'PUBLISHED',
                'STAGING_DEPLOYING',
                'STAGING_VERIFIED',
                'PRODUCTION_APPROVED',
                'PRODUCTION_DEPLOYING',
                'PRODUCTION_VERIFIED',
                'FAILED',
                'ROLLING_BACK',
                'ROLLED_BACK',
                'ROLLBACK_FAILED'
            )
        ),

    CONSTRAINT releases_project_version_unique
        UNIQUE (project_id, version)
);


CREATE INDEX releases_project_created_idx
    ON releases (project_id, created_at DESC);


CREATE INDEX releases_status_idx
    ON releases (status);


CREATE TABLE release_images (
    id UUID PRIMARY KEY,

    release_id UUID NOT NULL
        REFERENCES releases(id)
        ON DELETE CASCADE,

    repository TEXT NOT NULL,

    version_tag TEXT NOT NULL,
    git_sha_tag TEXT NOT NULL,

    target_platform TEXT NOT NULL,

    local_image_id TEXT,

    registry_digest TEXT,

    scan_result JSONB,

    published_at TIMESTAMPTZ,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT release_images_repository_not_empty
        CHECK (BTRIM(repository) <> ''),

    CONSTRAINT release_images_target_platform_not_empty
        CHECK (BTRIM(target_platform) <> ''),

    CONSTRAINT release_images_registry_digest_valid
        CHECK (
            registry_digest IS NULL
            OR registry_digest ~ '^sha256:[0-9a-f]{64}$'
        ),

    CONSTRAINT release_images_release_unique
        UNIQUE (release_id)
);


CREATE TABLE jobs (
    id UUID PRIMARY KEY,

    project_id UUID NOT NULL
        REFERENCES projects(id)
        ON DELETE RESTRICT,

    release_id UUID
        REFERENCES releases(id)
        ON DELETE SET NULL,

    environment_id UUID
        REFERENCES environments(id)
        ON DELETE SET NULL,

    job_type TEXT NOT NULL,

    status TEXT NOT NULL DEFAULT 'QUEUED',

    cancellation_requested BOOLEAN NOT NULL DEFAULT FALSE,

    attempt INTEGER NOT NULL DEFAULT 1,
    max_attempts INTEGER NOT NULL DEFAULT 1,

    worker_id TEXT,

    queued_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    claimed_at TIMESTAMPTZ,
    started_at TIMESTAMPTZ,
    heartbeat_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,

    error_code TEXT,
    error_message TEXT,

    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,

    CONSTRAINT jobs_type_valid
        CHECK (
            job_type IN (
                'PROJECT_INSPECTION',
                'PREPARE_RELEASE',
                'DEPLOY_STAGING',
                'DEPLOY_PRODUCTION',
                'ROLLBACK'
            )
        ),

    CONSTRAINT jobs_status_valid
        CHECK (
            status IN (
                'QUEUED',
                'CLAIMED',
                'RUNNING',
                'WAITING_FOR_APPROVAL',
                'CANCELLING',
                'CANCELLED',
                'SUCCEEDED',
                'FAILED',
                'INTERRUPTED',
                'RECOVERY_REQUIRED',
                'ROLLING_BACK',
                'ROLLED_BACK',
                'ROLLBACK_FAILED'
            )
        ),

    CONSTRAINT jobs_attempt_valid
        CHECK (
            attempt >= 1
            AND max_attempts >= 1
            AND attempt <= max_attempts
        )
);


CREATE INDEX jobs_status_queued_idx
    ON jobs (status, queued_at);


CREATE INDEX jobs_release_idx
    ON jobs (release_id);


CREATE INDEX jobs_environment_idx
    ON jobs (environment_id);


CREATE TABLE job_steps (
    id UUID PRIMARY KEY,

    job_id UUID NOT NULL
        REFERENCES jobs(id)
        ON DELETE CASCADE,

    step_key TEXT NOT NULL,
    step_order INTEGER NOT NULL,

    status TEXT NOT NULL DEFAULT 'PENDING',

    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,

    exit_code INTEGER,

    error_code TEXT,
    error_message TEXT,

    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,

    CONSTRAINT job_steps_key_not_empty
        CHECK (BTRIM(step_key) <> ''),

    CONSTRAINT job_steps_order_valid
        CHECK (step_order >= 0),

    CONSTRAINT job_steps_status_valid
        CHECK (
            status IN (
                'PENDING',
                'RUNNING',
                'SUCCEEDED',
                'FAILED',
                'SKIPPED',
                'CANCELLED',
                'UNKNOWN_EXTERNAL_STATE'
            )
        ),

    CONSTRAINT job_steps_job_key_unique
        UNIQUE (job_id, step_key),

    CONSTRAINT job_steps_job_order_unique
        UNIQUE (job_id, step_order)
);


CREATE INDEX job_steps_job_idx
    ON job_steps (job_id, step_order);


CREATE TABLE job_events (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,

    job_id UUID NOT NULL
        REFERENCES jobs(id)
        ON DELETE CASCADE,

    step_id UUID
        REFERENCES job_steps(id)
        ON DELETE SET NULL,

    stream TEXT NOT NULL,

    level TEXT NOT NULL,

    message TEXT NOT NULL,

    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT job_events_stream_valid
        CHECK (
            stream IN (
                'SYSTEM',
                'STDOUT',
                'STDERR'
            )
        ),

    CONSTRAINT job_events_level_valid
        CHECK (
            level IN (
                'TRACE',
                'DEBUG',
                'INFO',
                'WARN',
                'ERROR'
            )
        )
);


CREATE INDEX job_events_job_id_idx
    ON job_events (job_id, id);


CREATE INDEX job_events_step_id_idx
    ON job_events (step_id, id);


CREATE TABLE deployments (
    id UUID PRIMARY KEY,

    release_id UUID NOT NULL
        REFERENCES releases(id)
        ON DELETE RESTRICT,

    environment_id UUID NOT NULL
        REFERENCES environments(id)
        ON DELETE RESTRICT,

    job_id UUID
        REFERENCES jobs(id)
        ON DELETE SET NULL,

    status TEXT NOT NULL DEFAULT 'PENDING',

    candidate_digest TEXT NOT NULL,
    previous_digest TEXT,

    target_slot TEXT,

    started_at TIMESTAMPTZ,
    traffic_switched_at TIMESTAMPTZ,
    verified_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,

    failure_code TEXT,
    failure_message TEXT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT deployments_status_valid
        CHECK (
            status IN (
                'PENDING',
                'PREPARING',
                'BACKING_UP',
                'MIGRATING',
                'DEPLOYING',
                'VERIFYING',
                'SUCCEEDED',
                'FAILED',
                'ROLLING_BACK',
                'ROLLED_BACK',
                'ROLLBACK_FAILED'
            )
        ),

    CONSTRAINT deployments_candidate_digest_valid
        CHECK (
            candidate_digest ~ '^sha256:[0-9a-f]{64}$'
        ),

    CONSTRAINT deployments_previous_digest_valid
        CHECK (
            previous_digest IS NULL
            OR previous_digest ~ '^sha256:[0-9a-f]{64}$'
        ),

    CONSTRAINT deployments_slot_valid
        CHECK (
            target_slot IS NULL
            OR target_slot IN ('BLUE', 'GREEN')
        )
);


CREATE INDEX deployments_environment_created_idx
    ON deployments (environment_id, created_at DESC);


CREATE INDEX deployments_release_idx
    ON deployments (release_id);


CREATE UNIQUE INDEX deployments_one_active_per_environment_idx
    ON deployments (environment_id)
    WHERE status IN (
        'PREPARING',
        'BACKING_UP',
        'MIGRATING',
        'DEPLOYING',
        'VERIFYING',
        'ROLLING_BACK'
    );


CREATE TABLE deployment_checks (
    id UUID PRIMARY KEY,

    deployment_id UUID NOT NULL
        REFERENCES deployments(id)
        ON DELETE CASCADE,

    check_name TEXT NOT NULL,
    check_type TEXT NOT NULL,

    status TEXT NOT NULL,

    duration_ms BIGINT,

    details JSONB NOT NULL DEFAULT '{}'::JSONB,

    checked_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT deployment_checks_status_valid
        CHECK (
            status IN (
                'PENDING',
                'PASSED',
                'FAILED',
                'SKIPPED'
            )
        ),

    CONSTRAINT deployment_checks_duration_valid
        CHECK (
            duration_ms IS NULL
            OR duration_ms >= 0
        )
);


CREATE INDEX deployment_checks_deployment_idx
    ON deployment_checks (deployment_id, checked_at);


CREATE TABLE release_approvals (
    id UUID PRIMARY KEY,

    release_id UUID NOT NULL
        REFERENCES releases(id)
        ON DELETE RESTRICT,

    environment_id UUID NOT NULL
        REFERENCES environments(id)
        ON DELETE RESTRICT,

    approval_type TEXT NOT NULL,

    approved_by TEXT NOT NULL,

    confirmation_value TEXT NOT NULL,

    approved_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,

    CONSTRAINT release_approvals_type_valid
        CHECK (
            approval_type IN (
                'PRODUCTION_DEPLOYMENT'
            )
        )
);


CREATE INDEX release_approvals_release_idx
    ON release_approvals (release_id, approved_at DESC);


CREATE TABLE backups (
    id UUID PRIMARY KEY,

    deployment_id UUID
        REFERENCES deployments(id)
        ON DELETE SET NULL,

    environment_id UUID NOT NULL
        REFERENCES environments(id)
        ON DELETE RESTRICT,

    status TEXT NOT NULL DEFAULT 'PENDING',

    backup_type TEXT NOT NULL,

    location_reference TEXT,

    checksum TEXT,

    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,

    failure_message TEXT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT backups_status_valid
        CHECK (
            status IN (
                'PENDING',
                'RUNNING',
                'SUCCEEDED',
                'FAILED',
                'VERIFIED'
            )
        )
);


CREATE INDEX backups_environment_created_idx
    ON backups (environment_id, created_at DESC);


CREATE TABLE rollback_records (
    id UUID PRIMARY KEY,

    failed_deployment_id UUID NOT NULL
        REFERENCES deployments(id)
        ON DELETE RESTRICT,

    target_deployment_id UUID NOT NULL
        REFERENCES deployments(id)
        ON DELETE RESTRICT,

    job_id UUID
        REFERENCES jobs(id)
        ON DELETE SET NULL,

    status TEXT NOT NULL,

    reason TEXT,

    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT rollback_records_status_valid
        CHECK (
            status IN (
                'PENDING',
                'RUNNING',
                'SUCCEEDED',
                'FAILED'
            )
        )
);


CREATE TABLE audit_events (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,

    project_id UUID
        REFERENCES projects(id)
        ON DELETE SET NULL,

    release_id UUID
        REFERENCES releases(id)
        ON DELETE SET NULL,

    actor TEXT NOT NULL,

    action TEXT NOT NULL,

    entity_type TEXT NOT NULL,
    entity_id UUID,

    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT audit_events_actor_not_empty
        CHECK (BTRIM(actor) <> ''),

    CONSTRAINT audit_events_action_not_empty
        CHECK (BTRIM(action) <> '')
);


CREATE INDEX audit_events_project_idx
    ON audit_events (project_id, id DESC);


CREATE INDEX audit_events_release_idx
    ON audit_events (release_id, id DESC);
