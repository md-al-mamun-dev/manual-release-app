-- Drop the existing global unique constraints
ALTER TABLE projects
    DROP CONSTRAINT projects_name_unique,
    DROP CONSTRAINT projects_repository_path_unique;

-- Create partial unique indexes that ignore archived projects
CREATE UNIQUE INDEX projects_name_active_unique
    ON projects (name)
    WHERE archived_at IS NULL;

CREATE UNIQUE INDEX projects_repository_path_active_unique
    ON projects (repository_path)
    WHERE archived_at IS NULL;
