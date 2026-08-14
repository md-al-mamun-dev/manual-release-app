-- Add migration script here
ALTER TABLE environments
    ADD COLUMN archived_at TIMESTAMPTZ,

    ADD COLUMN server_architecture TEXT,

    ADD COLUMN ssh_identity_secret_ref TEXT,

    ADD COLUMN registry_credential_secret_ref TEXT,

    ADD COLUMN remote_env_file_path TEXT;


ALTER TABLE environments
    DROP CONSTRAINT environments_project_name_unique;

ALTER TABLE environments
    DROP CONSTRAINT environments_project_type_unique;


CREATE UNIQUE INDEX environments_active_project_name_unique
    ON environments (
        project_id,
        name
    )
    WHERE archived_at IS NULL;


CREATE UNIQUE INDEX environments_active_project_type_unique
    ON environments (
        project_id,
        environment_type
    )
    WHERE archived_at IS NULL;


CREATE INDEX environments_active_project_idx
    ON environments (
        project_id,
        created_at
    )
    WHERE archived_at IS NULL;


ALTER TABLE environments
    ADD CONSTRAINT environments_server_architecture_not_empty
        CHECK (
            server_architecture IS NULL
            OR BTRIM(server_architecture) <> ''
        ),

    ADD CONSTRAINT environments_ssh_identity_ref_not_empty
        CHECK (
            ssh_identity_secret_ref IS NULL
            OR BTRIM(ssh_identity_secret_ref) <> ''
        ),

    ADD CONSTRAINT environments_registry_ref_not_empty
        CHECK (
            registry_credential_secret_ref IS NULL
            OR BTRIM(registry_credential_secret_ref) <> ''
        ),

    ADD CONSTRAINT environments_remote_env_path_not_empty
        CHECK (
            remote_env_file_path IS NULL
            OR BTRIM(remote_env_file_path) <> ''
        );