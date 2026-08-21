CREATE TABLE project_build_configs (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    application_type TEXT NOT NULL,
    framework TEXT NOT NULL,
    runtime_version TEXT NOT NULL,
    package_manager TEXT NOT NULL,
    dockerfile_path TEXT,
    build_context TEXT,
    application_port INTEGER,
    health_endpoint TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT project_build_configs_app_type_valid CHECK (application_type IN ('NODE', 'PYTHON')),
    CONSTRAINT project_build_configs_framework_valid CHECK (framework IN ('NEXTJS', 'NESTJS', 'EXPRESS', 'FASTAPI', 'UNKNOWN')),
    CONSTRAINT project_build_configs_pkg_manager_valid CHECK (package_manager IN ('NPM', 'PNPM', 'YARN', 'UV', 'PIP')),
    CONSTRAINT project_build_configs_port_valid CHECK (application_port IS NULL OR (application_port > 0 AND application_port <= 65535))
);
CREATE UNIQUE INDEX project_build_configs_project_unique ON project_build_configs (project_id);
