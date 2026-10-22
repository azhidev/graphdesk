CREATE TABLE pages (
    id TEXT PRIMARY KEY NOT NULL CHECK(length(id) = 36),
    title TEXT NOT NULL CHECK(length(trim(title)) > 0),
    title_key TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;

CREATE TABLE blocks (
    id TEXT PRIMARY KEY NOT NULL CHECK(length(id) = 36),
    page_id TEXT NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    parent_id TEXT,
    position INTEGER NOT NULL,
    content TEXT NOT NULL DEFAULT '',
    task_status TEXT CHECK(task_status IN ('todo', 'doing', 'done', 'cancelled')),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE(id, page_id),
    FOREIGN KEY(parent_id, page_id) REFERENCES blocks(id, page_id) ON DELETE CASCADE
) STRICT;

CREATE UNIQUE INDEX blocks_root_order ON blocks(page_id, position) WHERE parent_id IS NULL;
CREATE UNIQUE INDEX blocks_child_order ON blocks(parent_id, position) WHERE parent_id IS NOT NULL;
CREATE INDEX blocks_page_parent ON blocks(page_id, parent_id, position);
CREATE INDEX blocks_tasks ON blocks(task_status) WHERE task_status IS NOT NULL;

-- An insert may refer to itself; a move may refer to a descendant.
CREATE TRIGGER blocks_no_cycle_insert BEFORE INSERT ON blocks WHEN NEW.parent_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'block-parent cycle') WHERE EXISTS (
        WITH RECURSIVE ancestors(id) AS (
            SELECT NEW.parent_id
            UNION
            SELECT b.parent_id FROM blocks b JOIN ancestors a ON b.id = a.id
            WHERE b.parent_id IS NOT NULL
        ) SELECT 1 FROM ancestors WHERE id = NEW.id
    );
END;
CREATE TRIGGER blocks_no_cycle_update BEFORE UPDATE OF parent_id ON blocks WHEN NEW.parent_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'block-parent cycle') WHERE EXISTS (
        WITH RECURSIVE ancestors(id) AS (
            SELECT NEW.parent_id
            UNION
            SELECT b.parent_id FROM blocks b JOIN ancestors a ON b.id = a.id
            WHERE b.parent_id IS NOT NULL
        ) SELECT 1 FROM ancestors WHERE id = NEW.id
    );
END;
CREATE TRIGGER blocks_stable_identity BEFORE UPDATE OF id, page_id ON blocks
WHEN NEW.id != OLD.id OR NEW.page_id != OLD.page_id
BEGIN SELECT RAISE(ABORT, 'block identity and page are immutable'); END;
CREATE TRIGGER pages_stable_identity BEFORE UPDATE OF id ON pages WHEN NEW.id != OLD.id
BEGIN SELECT RAISE(ABORT, 'page identity is immutable'); END;

CREATE TABLE page_references (
    source_block_id TEXT NOT NULL REFERENCES blocks(id) ON DELETE CASCADE,
    target_page_id TEXT NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    PRIMARY KEY(source_block_id, target_page_id)
) STRICT;
CREATE INDEX references_backlinks ON page_references(target_page_id, source_block_id);

-- Separate property tables preserve real foreign keys without polymorphic owner IDs.
CREATE TABLE page_properties (
    page_id TEXT NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    key TEXT NOT NULL CHECK(length(trim(key)) > 0),
    value_json TEXT NOT NULL CHECK(json_valid(value_json)),
    PRIMARY KEY(page_id, key)
) STRICT;
CREATE TABLE block_properties (
    block_id TEXT NOT NULL REFERENCES blocks(id) ON DELETE CASCADE,
    key TEXT NOT NULL CHECK(length(trim(key)) > 0),
    value_json TEXT NOT NULL CHECK(json_valid(value_json)),
    PRIMARY KEY(block_id, key)
) STRICT;

CREATE TABLE journals (
    page_id TEXT PRIMARY KEY NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    date TEXT NOT NULL UNIQUE CHECK(
        length(date) = 10 AND date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
        AND date(date, '+0 days') IS NOT NULL AND date(date, '+0 days') = date
    )
) STRICT;
