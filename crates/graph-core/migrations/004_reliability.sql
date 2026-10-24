-- Stable graph identity and monotonic text revisions, including delete/restore ABA.
CREATE TABLE graph_identity (
    singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
    id TEXT NOT NULL UNIQUE CHECK(length(id) = 36)
) STRICT;
CREATE TABLE block_revisions (
    block_id TEXT PRIMARY KEY NOT NULL,
    revision INTEGER NOT NULL CHECK(revision >= 0)
) STRICT;
INSERT INTO block_revisions SELECT id, 0 FROM blocks;
CREATE TRIGGER blocks_revision_insert AFTER INSERT ON blocks BEGIN
    INSERT INTO block_revisions VALUES(NEW.id, 0)
    ON CONFLICT(block_id) DO UPDATE SET revision = revision + 1;
END;
CREATE TRIGGER blocks_revision_text AFTER UPDATE OF content ON blocks
WHEN NEW.content != OLD.content BEGIN
    UPDATE block_revisions SET revision = revision + 1 WHERE block_id = NEW.id;
END;
CREATE TRIGGER blocks_revision_delete AFTER DELETE ON blocks BEGIN
    UPDATE block_revisions SET revision = revision + 1 WHERE block_id = OLD.id;
END;
