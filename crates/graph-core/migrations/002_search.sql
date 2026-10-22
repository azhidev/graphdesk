CREATE VIRTUAL TABLE pages_fts USING fts5(title, content='pages', content_rowid='rowid', tokenize='unicode61');
CREATE VIRTUAL TABLE blocks_fts USING fts5(content, content='blocks', content_rowid='rowid', tokenize='unicode61');

CREATE TRIGGER pages_search_insert AFTER INSERT ON pages BEGIN
    INSERT INTO pages_fts(rowid, title) VALUES (NEW.rowid, NEW.title);
END;
CREATE TRIGGER pages_search_delete AFTER DELETE ON pages BEGIN
    INSERT INTO pages_fts(pages_fts, rowid, title) VALUES ('delete', OLD.rowid, OLD.title);
END;
CREATE TRIGGER pages_search_update AFTER UPDATE OF title ON pages BEGIN
    INSERT INTO pages_fts(pages_fts, rowid, title) VALUES ('delete', OLD.rowid, OLD.title);
    INSERT INTO pages_fts(rowid, title) VALUES (NEW.rowid, NEW.title);
END;
CREATE TRIGGER blocks_search_insert AFTER INSERT ON blocks BEGIN
    INSERT INTO blocks_fts(rowid, content) VALUES (NEW.rowid, NEW.content);
END;
CREATE TRIGGER blocks_search_delete AFTER DELETE ON blocks BEGIN
    INSERT INTO blocks_fts(blocks_fts, rowid, content) VALUES ('delete', OLD.rowid, OLD.content);
END;
CREATE TRIGGER blocks_search_update AFTER UPDATE OF content ON blocks BEGIN
    INSERT INTO blocks_fts(blocks_fts, rowid, content) VALUES ('delete', OLD.rowid, OLD.content);
    INSERT INTO blocks_fts(rowid, content) VALUES (NEW.rowid, NEW.content);
END;

-- Index content that predates this migration.
INSERT INTO pages_fts(pages_fts) VALUES ('rebuild');
INSERT INTO blocks_fts(blocks_fts) VALUES ('rebuild');
