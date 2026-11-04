use rusqlite::{Connection, OptionalExtension};
use std::{collections::HashMap, sync::Mutex};

use crate::{
    benchmark::BenchResult,
    error::*,
    minecraft::Instance,
    paths,
};

/// SQLite storage for settings, instances and benchmark history.
/// Every method is synchronous and short; the lock is never held across an await.
pub struct Db(pub Mutex<Connection>);

impl Db {
    pub fn open() -> Result<Self> {
        std::fs::create_dir_all(paths::root())?;
        let c = Connection::open(paths::database())?;
        c.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;
             CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS instances(id TEXT PRIMARY KEY, data TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS benchmarks(id INTEGER PRIMARY KEY AUTOINCREMENT, instance_id TEXT NOT NULL, data TEXT NOT NULL);",
        )?;
        Ok(Db(Mutex::new(c)))
    }

    // ---- settings ----
    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let c = self.0.lock().unwrap();
        Ok(c.query_row("SELECT value FROM settings WHERE key=?1", [key], |r| r.get::<_, String>(0)).optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let c = self.0.lock().unwrap();
        c.execute(
            "INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [key, value],
        )?;
        Ok(())
    }

    pub fn all_settings(&self) -> Result<HashMap<String, String>> {
        let c = self.0.lock().unwrap();
        let mut st = c.prepare("SELECT key,value FROM settings")?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut m = HashMap::new();
        for r in rows {
            let (k, v) = r?;
            m.insert(k, v);
        }
        Ok(m)
    }

    // ---- instances ----
    pub fn list_instances(&self) -> Result<Vec<Instance>> {
        let c = self.0.lock().unwrap();
        let mut st = c.prepare("SELECT data FROM instances")?;
        let rows = st.query_map([], |r| r.get::<_, String>(0))?;
        let mut v = Vec::new();
        for r in rows {
            v.push(serde_json::from_str::<Instance>(&r?)?);
        }
        v.sort_by_key(|i| i.created_at);
        Ok(v)
    }

    pub fn get_instance(&self, id: &str) -> Result<Instance> {
        let c = self.0.lock().unwrap();
        let data: Option<String> = c
            .query_row("SELECT data FROM instances WHERE id=?1", [id], |r| r.get(0))
            .optional()?;
        match data {
            Some(d) => Ok(serde_json::from_str(&d)?),
            None => fail("Instance not found"),
        }
    }

    pub fn save_instance(&self, inst: &Instance) -> Result<()> {
        let c = self.0.lock().unwrap();
        c.execute(
            "INSERT INTO instances(id,data) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET data=excluded.data",
            [&inst.id, &serde_json::to_string(inst)?],
        )?;
        Ok(())
    }

    pub fn delete_instance(&self, id: &str) -> Result<()> {
        let c = self.0.lock().unwrap();
        c.execute("DELETE FROM instances WHERE id=?1", [id])?;
        c.execute("DELETE FROM benchmarks WHERE instance_id=?1", [id])?;
        Ok(())
    }

    // ---- benchmarks ----
    pub fn add_benchmark(&self, r: &BenchResult) -> Result<i64> {
        let c = self.0.lock().unwrap();
        c.execute(
            "INSERT INTO benchmarks(instance_id,data) VALUES(?1,?2)",
            [&r.instance_id, &serde_json::to_string(r)?],
        )?;
        Ok(c.last_insert_rowid())
    }

    pub fn list_benchmarks(&self, instance_id: Option<&str>) -> Result<Vec<BenchResult>> {
        let c = self.0.lock().unwrap();
        let mut st = c.prepare("SELECT id,instance_id,data FROM benchmarks ORDER BY id DESC")?;
        let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?;
        let mut v = Vec::new();
        for r in rows {
            let (id, inst, data) = r?;
            if instance_id.map_or(true, |w| w == inst) {
                let mut b: BenchResult = serde_json::from_str(&data)?;
                b.id = id;
                v.push(b);
            }
        }
        Ok(v)
    }

    pub fn delete_benchmark(&self, id: i64) -> Result<()> {
        self.0.lock().unwrap().execute("DELETE FROM benchmarks WHERE id=?1", [id])?;
        Ok(())
    }
}
