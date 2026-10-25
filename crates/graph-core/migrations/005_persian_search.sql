-- Derived contentful indexes; never rewrite canonical titles or block text.
CREATE VIRTUAL TABLE pages_persian_fts USING fts5(title, tokenize='unicode61');
CREATE VIRTUAL TABLE blocks_persian_fts USING fts5(content, tokenize='unicode61');
INSERT INTO pages_persian_fts(rowid,title) SELECT rowid,persian_normalize(title) FROM pages;
INSERT INTO blocks_persian_fts(rowid,content) SELECT rowid,persian_normalize(content) FROM blocks;
CREATE TRIGGER pages_persian_insert AFTER INSERT ON pages BEGIN
    INSERT INTO pages_persian_fts(rowid,title) VALUES(NEW.rowid,persian_normalize(NEW.title));
END;
CREATE TRIGGER pages_persian_update AFTER UPDATE OF title ON pages WHEN OLD.title IS NOT NEW.title BEGIN
    DELETE FROM pages_persian_fts WHERE rowid=OLD.rowid;
    INSERT INTO pages_persian_fts(rowid,title) VALUES(NEW.rowid,persian_normalize(NEW.title));
END;
CREATE TRIGGER pages_persian_delete AFTER DELETE ON pages BEGIN
    DELETE FROM pages_persian_fts WHERE rowid=OLD.rowid;
END;
CREATE TRIGGER blocks_persian_insert AFTER INSERT ON blocks BEGIN
    INSERT INTO blocks_persian_fts(rowid,content) VALUES(NEW.rowid,persian_normalize(NEW.content));
END;
CREATE TRIGGER blocks_persian_update AFTER UPDATE OF content ON blocks WHEN OLD.content IS NOT NEW.content BEGIN
    DELETE FROM blocks_persian_fts WHERE rowid=OLD.rowid;
    INSERT INTO blocks_persian_fts(rowid,content) VALUES(NEW.rowid,persian_normalize(NEW.content));
END;
CREATE TRIGGER blocks_persian_delete AFTER DELETE ON blocks BEGIN
    DELETE FROM blocks_persian_fts WHERE rowid=OLD.rowid;
END;
