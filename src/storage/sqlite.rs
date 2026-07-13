use crate::domain::metrics::Metrics;
use crate::presentation::summary::HourSummary;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_double, c_int, c_void};
use std::path::Path;
use std::ptr;

const SQLITE_OK: c_int = 0;
const SQLITE_ROW: c_int = 100;
const SQLITE_DONE: c_int = 101;
const SQLITE_NULL: c_int = 5;
const SQLITE_OPEN_READWRITE: c_int = 0x0000_0002;
const SQLITE_OPEN_CREATE: c_int = 0x0000_0004;
const SQLITE_OPEN_FULLMUTEX: c_int = 0x0001_0000;

#[repr(C)]
struct sqlite3 {
    _private: [u8; 0],
}

#[repr(C)]
struct sqlite3_stmt {
    _private: [u8; 0],
}

#[link(name = "sqlite3")]
unsafe extern "C" {
    fn sqlite3_open_v2(
        filename: *const c_char,
        ppdb: *mut *mut sqlite3,
        flags: c_int,
        z_vfs: *const c_char,
    ) -> c_int;
    fn sqlite3_close(db: *mut sqlite3) -> c_int;
    fn sqlite3_errmsg(db: *mut sqlite3) -> *const c_char;
    fn sqlite3_exec(
        db: *mut sqlite3,
        sql: *const c_char,
        callback: Option<unsafe extern "C" fn()>,
        arg: *mut c_void,
        errmsg: *mut *mut c_char,
    ) -> c_int;
    fn sqlite3_free(ptr: *mut c_void);
    fn sqlite3_prepare_v2(
        db: *mut sqlite3,
        sql: *const c_char,
        n_byte: c_int,
        pp_stmt: *mut *mut sqlite3_stmt,
        tail: *mut *const c_char,
    ) -> c_int;
    fn sqlite3_finalize(stmt: *mut sqlite3_stmt) -> c_int;
    fn sqlite3_step(stmt: *mut sqlite3_stmt) -> c_int;
    fn sqlite3_bind_int64(stmt: *mut sqlite3_stmt, index: c_int, value: i64) -> c_int;
    fn sqlite3_bind_double(stmt: *mut sqlite3_stmt, index: c_int, value: c_double) -> c_int;
    fn sqlite3_bind_text(
        stmt: *mut sqlite3_stmt,
        index: c_int,
        value: *const c_char,
        n: c_int,
        destructor: Option<unsafe extern "C" fn(*mut c_void)>,
    ) -> c_int;
    fn sqlite3_bind_null(stmt: *mut sqlite3_stmt, index: c_int) -> c_int;
    fn sqlite3_column_int64(stmt: *mut sqlite3_stmt, index: c_int) -> i64;
    fn sqlite3_column_double(stmt: *mut sqlite3_stmt, index: c_int) -> c_double;
    fn sqlite3_column_text(stmt: *mut sqlite3_stmt, index: c_int) -> *const c_char;
    fn sqlite3_column_type(stmt: *mut sqlite3_stmt, index: c_int) -> c_int;
}

type SqliteDestructor = Option<unsafe extern "C" fn(*mut c_void)>;

fn sqlite_transient() -> SqliteDestructor {
    unsafe { std::mem::transmute::<isize, SqliteDestructor>(-1) }
}

pub struct Store {
    db: *mut sqlite3,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self, String> {
        let filename = CString::new(path.to_string_lossy().as_bytes())
            .map_err(|_| "SQLite path contains a null byte".to_string())?;
        let mut db = ptr::null_mut();
        let flags = SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE | SQLITE_OPEN_FULLMUTEX;

        let rc = unsafe { sqlite3_open_v2(filename.as_ptr(), &mut db, flags, ptr::null()) };
        if rc != SQLITE_OK {
            let message = if db.is_null() {
                format!("failed to open {}", path.display())
            } else {
                unsafe { error_message(db) }
            };
            if !db.is_null() {
                unsafe {
                    sqlite3_close(db);
                }
            }
            return Err(message);
        }

        let store = Self { db };
        store.init()?;
        Ok(store)
    }

    fn init(&self) -> Result<(), String> {
        self.exec(
            "
            PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            CREATE TABLE IF NOT EXISTS samples (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                service_name TEXT NOT NULL DEFAULT 'default',
                pid INTEGER NOT NULL DEFAULT 0,
                epoch INTEGER NOT NULL,
                local_ts TEXT NOT NULL,
                local_hour TEXT NOT NULL,
                cpu_percent REAL NOT NULL,
                rss_mb REAL NOT NULL,
                vm_size_mb REAL NOT NULL,
                threads INTEGER NOT NULL,
                file_descriptors INTEGER NOT NULL DEFAULT 0,
                read_mb REAL,
                write_mb REAL,
                read_mb_per_sec REAL,
                write_mb_per_sec REAL,
                goroutines INTEGER,
                goroutine_growth_per_min REAL,
                cpu_capacity_percent REAL NOT NULL DEFAULT 0,
                system_cpu_percent REAL NOT NULL DEFAULT 0,
                rss_system_percent REAL NOT NULL DEFAULT 0,
                mem_total_mb REAL NOT NULL DEFAULT 0,
                mem_used_mb REAL NOT NULL DEFAULT 0,
                mem_available_mb REAL NOT NULL DEFAULT 0,
                load1 REAL NOT NULL DEFAULT 0,
                load5 REAL NOT NULL DEFAULT 0,
                load15 REAL NOT NULL DEFAULT 0,
                uptime_secs INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_samples_hour ON samples(local_hour);
            CREATE INDEX IF NOT EXISTS idx_samples_epoch ON samples(epoch);
            ",
        )?;
        self.add_column_if_missing(
            "ALTER TABLE samples ADD COLUMN service_name TEXT NOT NULL DEFAULT 'default'",
        )?;
        self.add_column_if_missing(
            "ALTER TABLE samples ADD COLUMN pid INTEGER NOT NULL DEFAULT 0",
        )?;
        self.add_column_if_missing(
            "ALTER TABLE samples ADD COLUMN file_descriptors INTEGER NOT NULL DEFAULT 0",
        )?;
        self.add_column_if_missing("ALTER TABLE samples ADD COLUMN read_mb_per_sec REAL")?;
        self.add_column_if_missing("ALTER TABLE samples ADD COLUMN write_mb_per_sec REAL")?;
        self.add_column_if_missing("ALTER TABLE samples ADD COLUMN goroutines INTEGER")?;
        self.add_column_if_missing("ALTER TABLE samples ADD COLUMN goroutine_growth_per_min REAL")?;
        self.add_column_if_missing(
            "ALTER TABLE samples ADD COLUMN cpu_capacity_percent REAL NOT NULL DEFAULT 0",
        )?;
        self.add_column_if_missing(
            "ALTER TABLE samples ADD COLUMN system_cpu_percent REAL NOT NULL DEFAULT 0",
        )?;
        self.add_column_if_missing(
            "ALTER TABLE samples ADD COLUMN rss_system_percent REAL NOT NULL DEFAULT 0",
        )?;
        self.add_column_if_missing(
            "ALTER TABLE samples ADD COLUMN mem_total_mb REAL NOT NULL DEFAULT 0",
        )?;
        self.add_column_if_missing(
            "ALTER TABLE samples ADD COLUMN mem_used_mb REAL NOT NULL DEFAULT 0",
        )?;
        self.add_column_if_missing(
            "ALTER TABLE samples ADD COLUMN mem_available_mb REAL NOT NULL DEFAULT 0",
        )?;
        self.add_column_if_missing("ALTER TABLE samples ADD COLUMN load1 REAL NOT NULL DEFAULT 0")?;
        self.add_column_if_missing("ALTER TABLE samples ADD COLUMN load5 REAL NOT NULL DEFAULT 0")?;
        self.add_column_if_missing(
            "ALTER TABLE samples ADD COLUMN load15 REAL NOT NULL DEFAULT 0",
        )?;
        self.add_column_if_missing(
            "ALTER TABLE samples ADD COLUMN uptime_secs INTEGER NOT NULL DEFAULT 0",
        )?;
        self.exec(
            "
            CREATE INDEX IF NOT EXISTS idx_samples_service_epoch
                ON samples(service_name, epoch);
            CREATE INDEX IF NOT EXISTS idx_samples_service_hour
                ON samples(service_name, local_hour);
            ",
        )
    }

    pub fn insert_sample(
        &self,
        service_name: &str,
        pid: i32,
        metrics: &Metrics,
    ) -> Result<(), String> {
        let sql = "
            INSERT INTO samples (
                service_name, pid, epoch, local_ts, local_hour, cpu_percent, rss_mb,
                vm_size_mb, threads, file_descriptors, read_mb, write_mb,
                read_mb_per_sec, write_mb_per_sec, goroutines, goroutine_growth_per_min,
                cpu_capacity_percent, system_cpu_percent, rss_system_percent,
                mem_total_mb, mem_used_mb, mem_available_mb,
                load1, load5, load15, uptime_secs
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ";
        let stmt = Statement::prepare(self.db, sql)?;
        stmt.bind_text(1, service_name)?;
        stmt.bind_i64(2, pid as i64)?;
        stmt.bind_i64(3, metrics.epoch as i64)?;
        stmt.bind_text(4, &metrics.local_ts)?;
        stmt.bind_text(5, &metrics.local_hour)?;
        stmt.bind_f64(6, metrics.cpu_percent)?;
        stmt.bind_f64(7, metrics.rss_mb)?;
        stmt.bind_f64(8, metrics.vm_size_mb)?;
        stmt.bind_i64(9, metrics.threads as i64)?;
        stmt.bind_i64(10, metrics.file_descriptors as i64)?;
        stmt.bind_optional_f64(11, metrics.read_mb)?;
        stmt.bind_optional_f64(12, metrics.write_mb)?;
        stmt.bind_optional_f64(13, metrics.read_mb_per_sec)?;
        stmt.bind_optional_f64(14, metrics.write_mb_per_sec)?;
        stmt.bind_optional_i64(15, metrics.goroutines.map(|value| value as i64))?;
        stmt.bind_optional_f64(16, metrics.goroutine_growth_per_min)?;
        stmt.bind_f64(17, metrics.cpu_capacity_percent)?;
        stmt.bind_f64(18, metrics.system_cpu_percent)?;
        stmt.bind_f64(19, metrics.rss_system_percent)?;
        stmt.bind_f64(20, metrics.mem_total_mb)?;
        stmt.bind_f64(21, metrics.mem_used_mb)?;
        stmt.bind_f64(22, metrics.mem_available_mb)?;
        stmt.bind_f64(23, metrics.load1)?;
        stmt.bind_f64(24, metrics.load5)?;
        stmt.bind_f64(25, metrics.load15)?;
        stmt.bind_i64(26, metrics.uptime_secs as i64)?;
        stmt.step_done()
    }

    pub fn prune(
        &self,
        retention_hours: u64,
        max_samples: u64,
        now_epoch: u64,
    ) -> Result<(), String> {
        if retention_hours > 0 {
            let cutoff = now_epoch.saturating_sub(retention_hours.saturating_mul(3600));
            let stmt = Statement::prepare(self.db, "DELETE FROM samples WHERE epoch < ?")?;
            stmt.bind_i64(1, cutoff as i64)?;
            stmt.step_done()?;
        }

        if max_samples > 0 {
            let stmt = Statement::prepare(
                self.db,
                "
                DELETE FROM samples
                WHERE id IN (
                    SELECT id FROM (
                        SELECT
                            id,
                            ROW_NUMBER() OVER (
                                PARTITION BY service_name
                                ORDER BY id DESC
                            ) AS service_row
                        FROM samples
                    )
                    WHERE service_row > ?
                )
                ",
            )?;
            stmt.bind_i64(1, max_samples as i64)?;
            stmt.step_done()?;
        }

        Ok(())
    }

    pub fn hourly_summary(&self, service: Option<&str>) -> Result<Vec<HourSummary>, String> {
        let sql_all = "
            SELECT
                service_name,
                local_hour,
                COUNT(*) AS samples,
                AVG(cpu_percent) AS cpu_avg,
                MAX(cpu_percent) AS cpu_peak,
                AVG(rss_mb) AS rss_avg,
                MAX(rss_mb) AS rss_peak,
                AVG(goroutines) AS goroutines_avg,
                MAX(goroutines) AS goroutines_peak,
                MAX(file_descriptors) AS file_descriptors_peak
            FROM samples
            GROUP BY service_name, local_hour
            ORDER BY local_hour, service_name
        ";
        let sql_service = "
            SELECT
                service_name,
                local_hour,
                COUNT(*) AS samples,
                AVG(cpu_percent) AS cpu_avg,
                MAX(cpu_percent) AS cpu_peak,
                AVG(rss_mb) AS rss_avg,
                MAX(rss_mb) AS rss_peak,
                AVG(goroutines) AS goroutines_avg,
                MAX(goroutines) AS goroutines_peak,
                MAX(file_descriptors) AS file_descriptors_peak
            FROM samples
            WHERE service_name = ?
            GROUP BY service_name, local_hour
            ORDER BY local_hour
        ";
        let stmt = Statement::prepare(
            self.db,
            if service.is_some() {
                sql_service
            } else {
                sql_all
            },
        )?;
        if let Some(service) = service {
            stmt.bind_text(1, service)?;
        }
        let mut rows = Vec::new();

        while let Step::Row = stmt.step()? {
            rows.push(HourSummary {
                service_name: stmt.column_text(0),
                local_hour: stmt.column_text(1),
                samples: stmt.column_i64(2),
                cpu_avg: stmt.column_f64(3),
                cpu_peak: stmt.column_f64(4),
                rss_avg: stmt.column_f64(5),
                rss_peak: stmt.column_f64(6),
                goroutines_avg: stmt.column_optional_f64(7),
                goroutines_peak: stmt.column_optional_i64(8),
                file_descriptors_peak: stmt.column_i64(9),
            });
        }

        Ok(rows)
    }

    fn exec(&self, sql: &str) -> Result<(), String> {
        let sql = CString::new(sql).map_err(|_| "SQL contains a null byte".to_string())?;
        let mut errmsg = ptr::null_mut();
        let rc = unsafe { sqlite3_exec(self.db, sql.as_ptr(), None, ptr::null_mut(), &mut errmsg) };

        if rc == SQLITE_OK {
            return Ok(());
        }

        let message = if errmsg.is_null() {
            unsafe { error_message(self.db) }
        } else {
            let message = unsafe { CStr::from_ptr(errmsg).to_string_lossy().into_owned() };
            unsafe {
                sqlite3_free(errmsg as *mut c_void);
            }
            message
        };
        Err(message)
    }

    fn add_column_if_missing(&self, sql: &str) -> Result<(), String> {
        match self.exec(sql) {
            Ok(()) => Ok(()),
            Err(err) if err.contains("duplicate column name") => Ok(()),
            Err(err) => Err(err),
        }
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        if !self.db.is_null() {
            unsafe {
                sqlite3_close(self.db);
            }
        }
    }
}

enum Step {
    Row,
    Done,
}

struct Statement {
    db: *mut sqlite3,
    stmt: *mut sqlite3_stmt,
}

impl Statement {
    fn prepare(db: *mut sqlite3, sql: &str) -> Result<Self, String> {
        let sql = CString::new(sql).map_err(|_| "SQL contains a null byte".to_string())?;
        let mut stmt = ptr::null_mut();
        let rc = unsafe { sqlite3_prepare_v2(db, sql.as_ptr(), -1, &mut stmt, ptr::null_mut()) };
        if rc != SQLITE_OK {
            return Err(unsafe { error_message(db) });
        }
        Ok(Self { db, stmt })
    }

    fn bind_i64(&self, index: c_int, value: i64) -> Result<(), String> {
        self.check(unsafe { sqlite3_bind_int64(self.stmt, index, value) })
    }

    fn bind_f64(&self, index: c_int, value: f64) -> Result<(), String> {
        self.check(unsafe { sqlite3_bind_double(self.stmt, index, value) })
    }

    fn bind_optional_f64(&self, index: c_int, value: Option<f64>) -> Result<(), String> {
        match value {
            Some(value) => self.bind_f64(index, value),
            None => self.check(unsafe { sqlite3_bind_null(self.stmt, index) }),
        }
    }

    fn bind_optional_i64(&self, index: c_int, value: Option<i64>) -> Result<(), String> {
        match value {
            Some(value) => self.bind_i64(index, value),
            None => self.check(unsafe { sqlite3_bind_null(self.stmt, index) }),
        }
    }

    fn bind_text(&self, index: c_int, value: &str) -> Result<(), String> {
        let value = CString::new(value).map_err(|_| "text contains a null byte".to_string())?;
        self.check(unsafe {
            sqlite3_bind_text(self.stmt, index, value.as_ptr(), -1, sqlite_transient())
        })
    }

    fn step(&self) -> Result<Step, String> {
        match unsafe { sqlite3_step(self.stmt) } {
            SQLITE_ROW => Ok(Step::Row),
            SQLITE_DONE => Ok(Step::Done),
            _ => Err(unsafe { error_message(self.db) }),
        }
    }

    fn step_done(&self) -> Result<(), String> {
        match self.step()? {
            Step::Done => Ok(()),
            Step::Row => Err("SQLite returned a row where completion was expected".to_string()),
        }
    }

    fn column_i64(&self, index: c_int) -> i64 {
        unsafe { sqlite3_column_int64(self.stmt, index) }
    }

    fn column_f64(&self, index: c_int) -> f64 {
        unsafe { sqlite3_column_double(self.stmt, index) }
    }

    fn column_optional_i64(&self, index: c_int) -> Option<i64> {
        (unsafe { sqlite3_column_type(self.stmt, index) } != SQLITE_NULL)
            .then(|| self.column_i64(index))
    }

    fn column_optional_f64(&self, index: c_int) -> Option<f64> {
        (unsafe { sqlite3_column_type(self.stmt, index) } != SQLITE_NULL)
            .then(|| self.column_f64(index))
    }

    fn column_text(&self, index: c_int) -> String {
        let ptr = unsafe { sqlite3_column_text(self.stmt, index) };
        if ptr.is_null() {
            String::new()
        } else {
            unsafe { CStr::from_ptr(ptr as *const c_char) }
                .to_string_lossy()
                .into_owned()
        }
    }

    fn check(&self, rc: c_int) -> Result<(), String> {
        if rc == SQLITE_OK {
            Ok(())
        } else {
            Err(unsafe { error_message(self.db) })
        }
    }
}

impl Drop for Statement {
    fn drop(&mut self) {
        if !self.stmt.is_null() {
            unsafe {
                sqlite3_finalize(self.stmt);
            }
        }
    }
}

unsafe fn error_message(db: *mut sqlite3) -> String {
    let ptr = unsafe { sqlite3_errmsg(db) };
    if ptr.is_null() {
        "unknown SQLite error".to_string()
    } else {
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_ID: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn stores_and_prunes_samples_per_service() {
        let path = test_database_path();
        {
            let store = Store::open(&path).unwrap();
            for epoch in 1..=3 {
                store.insert_sample("api", 10, &metrics(epoch)).unwrap();
                store.insert_sample("worker", 20, &metrics(epoch)).unwrap();
            }
            store.prune(0, 2, 3).unwrap();
            let rows = store.hourly_summary(None).unwrap();
            assert_eq!(rows.len(), 2);
            assert!(rows.iter().all(|row| row.samples == 2));
            assert!(rows.iter().any(|row| row.service_name == "api"));
            assert!(rows.iter().any(|row| row.service_name == "worker"));
        }
        remove_database_files(&path);
    }

    #[test]
    fn filters_summary_by_service() {
        let path = test_database_path();
        {
            let store = Store::open(&path).unwrap();
            store.insert_sample("api", 10, &metrics(1)).unwrap();
            store.insert_sample("worker", 20, &metrics(1)).unwrap();
            let rows = store.hourly_summary(Some("api")).unwrap();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].service_name, "api");
        }
        remove_database_files(&path);
    }

    fn metrics(epoch: u64) -> Metrics {
        Metrics {
            epoch,
            local_ts: "2026-01-01 00:00:00".into(),
            local_hour: "2026-01-01 00:00".into(),
            cpu_percent: 10.0,
            cpu_capacity_percent: 5.0,
            system_cpu_percent: 20.0,
            rss_mb: 32.0,
            rss_system_percent: 3.2,
            vm_size_mb: 64.0,
            threads: 5,
            file_descriptors: 12,
            read_mb: Some(1.0),
            write_mb: Some(2.0),
            read_mb_per_sec: Some(0.1),
            write_mb_per_sec: Some(0.2),
            goroutines: Some(20),
            goroutine_growth_per_min: Some(0.0),
            mem_total_mb: 1024.0,
            mem_used_mb: 512.0,
            mem_available_mb: 512.0,
            load1: 0.1,
            load5: 0.2,
            load15: 0.3,
            uptime_secs: epoch,
        }
    }

    fn test_database_path() -> std::path::PathBuf {
        let id = TEST_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("gsw-test-{}-{id}.db", std::process::id()))
    }

    fn remove_database_files(path: &Path) {
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(format!("{}-wal", path.display()));
        let _ = fs::remove_file(format!("{}-shm", path.display()));
    }
}
