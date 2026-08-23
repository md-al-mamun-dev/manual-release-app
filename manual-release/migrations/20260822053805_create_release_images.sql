DROP TABLE IF EXISTS release_images CASCADE;

CREATE TABLE release_images (
    id UUID PRIMARY KEY,
    release_id UUID NOT NULL
        REFERENCES releases(id)
        ON DELETE CASCADE,
    job_id UUID NOT NULL
        REFERENCES jobs(id)
        ON DELETE CASCADE,
    git_sha TEXT NOT NULL,
    image_tag TEXT NOT NULL,
    image_digest TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_release_images_release_id ON release_images(release_id);
CREATE INDEX idx_release_images_job_id ON release_images(job_id);
