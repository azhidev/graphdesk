-- Dedicated, unbounded history: never stored in the generic extension document.
CREATE TABLE habits (
    graph_id TEXT NOT NULL REFERENCES graph_identity(id),
    id TEXT NOT NULL CHECK(length(id)=36),
    revision INTEGER NOT NULL DEFAULT 0 CHECK(revision BETWEEN 0 AND 9007199254740991),
    definition_json TEXT NOT NULL CHECK(json_valid(definition_json)),
    PRIMARY KEY(graph_id,id)
) STRICT;
-- UTC edit audit preserves historical anchors without deriving analytics now.
CREATE TABLE habit_versions (
    graph_id TEXT NOT NULL REFERENCES graph_identity(id),
    habit_id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK(revision BETWEEN 0 AND 9007199254740991),
    definition_json TEXT NOT NULL CHECK(json_valid(definition_json)),
    recorded_at INTEGER NOT NULL DEFAULT(unixepoch()),
    PRIMARY KEY(graph_id,habit_id,revision),
    FOREIGN KEY(graph_id,habit_id) REFERENCES habits(graph_id,id)
) STRICT;
CREATE TABLE habit_events (
    graph_id TEXT NOT NULL REFERENCES graph_identity(id),
    id TEXT NOT NULL CHECK(length(id)=36),
    habit_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('completion','lapse','confirmation')),
    -- Definition known at recording, not guessed from a backdated occurrence.
    habit_revision INTEGER NOT NULL CHECK(habit_revision BETWEEN 0 AND 9007199254740991),
    occurred_at INTEGER NOT NULL CHECK(occurred_at BETWEEN -62135596800 AND 253402300799),
    recorded_at INTEGER NOT NULL CHECK(recorded_at BETWEEN occurred_at AND 253402300799),
    period_start INTEGER CHECK(period_start BETWEEN -62135596800 AND 253402300799),
    period_end INTEGER CHECK(period_end BETWEEN -62135596800 AND 253402300799),
    revision INTEGER NOT NULL DEFAULT 0 CHECK(revision BETWEEN 0 AND 9007199254740991),
    voided INTEGER NOT NULL DEFAULT 0 CHECK(voided IN (0,1)),
    PRIMARY KEY(graph_id,id),
    FOREIGN KEY(graph_id,habit_id) REFERENCES habits(graph_id,id),
    FOREIGN KEY(graph_id,habit_id,habit_revision) REFERENCES habit_versions(graph_id,habit_id,revision),
    CHECK((kind='confirmation' AND period_start IS NOT NULL AND period_end IS NOT NULL
        AND period_start < period_end AND period_end <= occurred_at)
        OR (kind IN ('completion','lapse') AND period_start IS NULL AND period_end IS NULL))
) STRICT;
CREATE INDEX habit_events_history ON habit_events(graph_id,habit_id,occurred_at DESC,id DESC);
CREATE INDEX habit_events_status ON habit_events(graph_id,habit_id,kind,occurred_at DESC,id DESC) WHERE voided=0;
CREATE TRIGGER habit_events_immutable BEFORE UPDATE OF graph_id,id,habit_id,habit_revision,kind,occurred_at,recorded_at,period_start,period_end ON habit_events
BEGIN SELECT RAISE(ABORT,'habit event payload is immutable'); END;
