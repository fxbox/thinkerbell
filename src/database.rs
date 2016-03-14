use rusqlite;
use std::path::Path;
use std::error::Error;

pub struct StoredScript {
    pub id: Option<i64>,
    pub name: String,
    pub source: String,
    pub is_active: bool
}

pub struct ScriptDatabase {
    connection: rusqlite::Connection
}

impl ScriptDatabase {

    pub fn new(path: &Path) -> Result<Self, Box<Error>> {
        let connection = try!(rusqlite::Connection::open(path));
        try!(connection.execute("CREATE TABLE IF NOT EXISTS scripts (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL UNIQUE,
            source      TEXT NOT NULL,
            is_active   BOOL NOT NULL DEFAULT 1
        )", &[]));

        Ok(ScriptDatabase {
            connection: connection
        })
    }

    pub fn insert(&self, script: &mut StoredScript) -> rusqlite::Result<()> {
        if let Some(id) = script.id {
            self.connection.execute("UPDATE scripts SET name = $1, source = $2, is_active = $3 WHERE id = $4",
                &[&script.name, &script.source, &script.is_active, &id])
                .map(|_| ())
        } else {
            self.connection.execute("INSERT INTO SCRIPTS (name, source, is_active) VALUES ($1, $2, $3)",
                &[&script.name, &script.source, &script.is_active])
                .map(|_| {
                    script.id = Some(self.connection.last_insert_rowid());
                })
        }
    }

    pub fn get_all(&self) -> Vec<StoredScript> {
        let mut stmt = self.connection.prepare("SELECT id, name, source, is_active FROM scripts").unwrap();
        stmt.query_map(&[], |row| {
            StoredScript {
                id: row.get(0),
                name: row.get(1),
                source: row.get(2),
                is_active: row.get(3)
            }
        }).unwrap().map(|x| x.unwrap()).collect()
    }

    pub fn remove(&self, id: i64) -> rusqlite::Result<()> {
        self.connection.execute("DELETE FROM scripts WHERE id = $1", &[&id]).map(|_| ())
    }

    pub fn remove_all(&self) -> rusqlite::Result<()> {
        self.connection.execute("DELETE FROM scripts", &[]).map(|_| ())
    }
}
