-- Parsed inline links are distinct from explicit API-created reference edges.
-- Resolved UUID bindings survive title changes; unresolved labels remain visible.
CREATE TABLE inline_references (
    source_block_id TEXT NOT NULL REFERENCES blocks(id) ON DELETE CASCADE,
    title_key TEXT NOT NULL,
    label TEXT NOT NULL,
    target_page_id TEXT REFERENCES pages(id) ON DELETE SET NULL,
    PRIMARY KEY(source_block_id, title_key)
) STRICT;
CREATE INDEX inline_backlinks ON inline_references(target_page_id, source_block_id);
