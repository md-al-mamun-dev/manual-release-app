-- Add migration script here
CREATE TABLE project_inspections (
    id UUID PRIMARY KEY,

    project_id UUID NOT NULL
        REFERENCES projects(id)
        ON DELETE CASCADE,

    status TEXT NOT NULL DEFAULT 'RUNNING',

    canonical_repository_path TEXT,

    git_commit TEXT,
    git_branch TEXT,
    git_dirty BOOLEAN,

    report JSONB,

    error_code TEXT,
    error_message TEXT,

    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMPTZ,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT project_inspections_status_valid
        CHECK (
            status IN (
                'RUNNING',
                'SUCCEEDED',
                'FAILED'
            )
        ),

    CONSTRAINT project_inspections_git_commit_valid
        CHECK (
            git_commit IS NULL
            OR git_commit ~ '^[0-9a-f]{40}([0-9a-f]{24})?$'
        ),

    CONSTRAINT project_inspections_finished_state_valid
        CHECK (
            (
                status = 'RUNNING'
                AND finished_at IS NULL
            )
            OR
            (
                status IN ('SUCCEEDED', 'FAILED')
                AND finished_at IS NOT NULL
            )
        )
);


CREATE INDEX project_inspections_project_created_idx
    ON project_inspections (
        project_id,
        created_at DESC
    );