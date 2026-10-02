use rusqlite_migration::{Migrations, M};

pub const MIGRATIONS: Migrations<'static> = Migrations::from_slice(MIGRATION_LIST);

/// Append new migrations to the end; never edit one that has shipped.
const MIGRATION_LIST: &[M<'static>] = &[
    // 1: core schema. `folder_id IS NULL` means the bookmark is unfiled.
    M::up(
        r#"
        CREATE TABLE folders (
            id             INTEGER PRIMARY KEY,
            parent_id      INTEGER REFERENCES folders(id) ON DELETE CASCADE,
            name           TEXT NOT NULL,
            position       INTEGER NOT NULL,
            default_target TEXT
        );
        CREATE INDEX folders_parent_position ON folders(parent_id, position);

        CREATE TABLE bookmarks (
            id             INTEGER PRIMARY KEY,
            folder_id      INTEGER REFERENCES folders(id) ON DELETE CASCADE,
            title          TEXT NOT NULL DEFAULT '',
            url            TEXT NOT NULL,
            notes          TEXT NOT NULL DEFAULT '',
            favicon        TEXT,
            position       INTEGER NOT NULL,
            default_target TEXT,
            created_at     INTEGER NOT NULL,
            updated_at     INTEGER NOT NULL,
            last_opened_at INTEGER,
            open_count     INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX bookmarks_folder_position ON bookmarks(folder_id, position);

        CREATE TABLE tags (
            id   INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE COLLATE NOCASE
        );

        CREATE TABLE bookmark_tags (
            bookmark_id INTEGER NOT NULL REFERENCES bookmarks(id) ON DELETE CASCADE,
            tag_id      INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
            PRIMARY KEY (bookmark_id, tag_id)
        ) WITHOUT ROWID;
        CREATE INDEX bookmark_tags_tag ON bookmark_tags(tag_id);

        CREATE TABLE settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        ) WITHOUT ROWID;
        "#,
    ),
    // 2: full-text search. The trigram tokenizer gives case-insensitive substring
    // matching. rowid = bookmarks.id; `tags` holds the bookmark's tag names, space-separated.
    M::up(
        r#"
        CREATE VIRTUAL TABLE bookmarks_fts USING fts5(
            title, url, notes, tags,
            tokenize = 'trigram'
        );

        INSERT INTO bookmarks_fts (rowid, title, url, notes, tags)
        SELECT b.id, b.title, b.url, b.notes,
               COALESCE((SELECT group_concat(t.name, ' ')
                         FROM bookmark_tags bt JOIN tags t ON t.id = bt.tag_id
                         WHERE bt.bookmark_id = b.id), '')
        FROM bookmarks b;

        CREATE TRIGGER bookmarks_fts_insert AFTER INSERT ON bookmarks BEGIN
            INSERT INTO bookmarks_fts (rowid, title, url, notes, tags)
            VALUES (new.id, new.title, new.url, new.notes, '');
        END;

        CREATE TRIGGER bookmarks_fts_update AFTER UPDATE OF title, url, notes ON bookmarks BEGIN
            UPDATE bookmarks_fts SET title = new.title, url = new.url, notes = new.notes
            WHERE rowid = new.id;
        END;

        CREATE TRIGGER bookmarks_fts_delete AFTER DELETE ON bookmarks BEGIN
            DELETE FROM bookmarks_fts WHERE rowid = old.id;
        END;

        CREATE TRIGGER bookmark_tags_fts_insert AFTER INSERT ON bookmark_tags BEGIN
            UPDATE bookmarks_fts SET tags = COALESCE((
                SELECT group_concat(t.name, ' ')
                FROM bookmark_tags bt JOIN tags t ON t.id = bt.tag_id
                WHERE bt.bookmark_id = new.bookmark_id), '')
            WHERE rowid = new.bookmark_id;
        END;

        CREATE TRIGGER bookmark_tags_fts_delete AFTER DELETE ON bookmark_tags BEGIN
            UPDATE bookmarks_fts SET tags = COALESCE((
                SELECT group_concat(t.name, ' ')
                FROM bookmark_tags bt JOIN tags t ON t.id = bt.tag_id
                WHERE bt.bookmark_id = old.bookmark_id), '')
            WHERE rowid = old.bookmark_id;
        END;

        CREATE TRIGGER tags_fts_rename AFTER UPDATE OF name ON tags BEGIN
            UPDATE bookmarks_fts SET tags = COALESCE((
                SELECT group_concat(t.name, ' ')
                FROM bookmark_tags bt JOIN tags t ON t.id = bt.tag_id
                WHERE bt.bookmark_id = bookmarks_fts.rowid), '')
            WHERE rowid IN (SELECT bookmark_id FROM bookmark_tags WHERE tag_id = new.id);
        END;
        "#,
    ),
];
