-- Add security scanning states to releases status check constraint
ALTER TABLE releases DROP CONSTRAINT IF EXISTS releases_status_valid;

ALTER TABLE releases
    ADD CONSTRAINT releases_status_valid
        CHECK (
            status IN (
                'CREATED',
                'SOURCE_VALIDATED',
                'CI_RUNNING',
                'CI_PASSED',
                'IMAGE_BUILDING',
                'IMAGE_BUILT',
                'IMAGE_TESTING',
                'IMAGE_TESTED',
                'SECURITY_SCANNING',
                'SCAN_PASSED',
                'SECURITY_FAILED',
                'IMAGE_APPROVED',
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
        );

-- Create security_scans table
CREATE TABLE security_scans (
    id UUID PRIMARY KEY,
    release_id UUID NOT NULL
        REFERENCES releases(id)
        ON DELETE CASCADE,
    image_digest TEXT NOT NULL,
    critical_vulnerabilities INTEGER NOT NULL DEFAULT 0,
    high_vulnerabilities INTEGER NOT NULL DEFAULT 0,
    medium_vulnerabilities INTEGER NOT NULL DEFAULT 0,
    low_vulnerabilities INTEGER NOT NULL DEFAULT 0,
    passed BOOLEAN NOT NULL,
    report_json JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT security_scans_critical_non_negative CHECK (critical_vulnerabilities >= 0),
    CONSTRAINT security_scans_high_non_negative CHECK (high_vulnerabilities >= 0),
    CONSTRAINT security_scans_medium_non_negative CHECK (medium_vulnerabilities >= 0),
    CONSTRAINT security_scans_low_non_negative CHECK (low_vulnerabilities >= 0),
    CONSTRAINT security_scans_image_digest_not_empty CHECK (BTRIM(image_digest) <> '')
);

CREATE INDEX idx_security_scans_release_id ON security_scans(release_id);
CREATE INDEX idx_security_scans_release_created ON security_scans(release_id, created_at DESC);
