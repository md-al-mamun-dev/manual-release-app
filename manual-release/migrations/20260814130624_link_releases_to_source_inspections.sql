ALTER TABLE releases
    ADD COLUMN source_inspection_id UUID
        REFERENCES project_inspections(id)
        ON DELETE RESTRICT;


CREATE INDEX releases_source_inspection_idx
    ON releases (source_inspection_id);


CREATE TABLE release_state_transitions (
    id BIGINT
        GENERATED ALWAYS AS IDENTITY
        PRIMARY KEY,

    release_id UUID NOT NULL
        REFERENCES releases(id)
        ON DELETE CASCADE,

    from_status TEXT,

    to_status TEXT NOT NULL,

    actor TEXT NOT NULL,

    reason TEXT,

    metadata JSONB NOT NULL
        DEFAULT '{}'::JSONB,

    created_at TIMESTAMPTZ NOT NULL
        DEFAULT NOW(),

    CONSTRAINT release_state_transitions_actor_not_empty
        CHECK (BTRIM(actor) <> ''),

    CONSTRAINT release_state_transitions_to_status_not_empty
        CHECK (BTRIM(to_status) <> '')
);


CREATE INDEX release_state_transitions_release_idx
    ON release_state_transitions (
        release_id,
        id
    );