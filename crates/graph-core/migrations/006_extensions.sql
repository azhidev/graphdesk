-- One bounded document per stable extension ID, scoped to this graph's identity.
-- Disabling an extension changes only its preference, never its retained data.
CREATE TABLE extensions (
    graph_id TEXT NOT NULL REFERENCES graph_identity(id),
    extension_id TEXT NOT NULL CHECK(
        length(CAST(extension_id AS BLOB)) BETWEEN 3 AND 100
        AND instr(extension_id, char(0)) = 0
        AND extension_id NOT GLOB '*[^a-z0-9.-]*'
        AND instr(extension_id, '.') > 0
        AND extension_id NOT LIKE '.%' AND extension_id NOT LIKE '%.'
        AND instr(extension_id, '..') = 0
        AND instr('.' || extension_id, '.-') = 0
        AND instr(extension_id || '.', '-.') = 0
    ),
    enabled INTEGER NOT NULL DEFAULT 0 CHECK(enabled IN (0, 1)),
    revision INTEGER NOT NULL DEFAULT 0 CHECK(revision >= 0),
    value_json TEXT NOT NULL DEFAULT 'null' CHECK(
        json_valid(value_json) AND length(CAST(value_json AS BLOB)) <= 16384
    ),
    PRIMARY KEY(graph_id, extension_id)
) STRICT;
