ALTER TABLE release_images
ADD COLUMN registry TEXT,
ADD COLUMN repository TEXT,
ADD COLUMN remote_digest TEXT,
ADD COLUMN publication_status TEXT,
ADD COLUMN updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW();
