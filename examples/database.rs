#![allow(unsafe_code)]

use std::ffi::CString;
use std::sync::Mutex;

use log::{debug, error, info, Level};
use trivial_log::Appender;

use libsqlite3_sys as ffi;

#[derive(Debug)]
struct SqliteLog {
  db: Mutex<*mut ffi::sqlite3>,
}

unsafe impl Send for SqliteLog {}
unsafe impl Sync for SqliteLog {}

impl Drop for SqliteLog {
  fn drop(&mut self) {
    let db = *self.db.lock().unwrap();
    if !db.is_null() {
      unsafe {
        ffi::sqlite3_close(db);
      }
    }
  }
}

impl Appender<LogEntity> for SqliteLog {
  fn append_log_message(&self, le: &LogEntity) {
    let db = *self.db.lock().unwrap();
    unsafe {
      let sql = CString::new("INSERT INTO logs (ts, level, log) VALUES (?, ?, ?)").unwrap();
      let mut stmt: *mut ffi::sqlite3_stmt = std::ptr::null_mut();

      let rc = ffi::sqlite3_prepare_v2(db, sql.as_ptr(), -1, &mut stmt, std::ptr::null_mut());
      if rc != ffi::SQLITE_OK {
        panic!("prepare: {}", std::ffi::CStr::from_ptr(ffi::sqlite3_errmsg(db)).to_string_lossy());
      }

      ffi::sqlite3_bind_int64(stmt, 1, i64::try_from(le.unix_time).unwrap_or(0));
      ffi::sqlite3_bind_int64(stmt, 2, i64::try_from(le.level as usize).unwrap_or(1));

      let msg = CString::new(le.message.as_str()).unwrap();
      ffi::sqlite3_bind_text(stmt, 3, msg.as_ptr(), -1, ffi::SQLITE_TRANSIENT());

      let rc = ffi::sqlite3_step(stmt);
      ffi::sqlite3_finalize(stmt);
      if rc != ffi::SQLITE_DONE {
        panic!("step: {}", std::ffi::CStr::from_ptr(ffi::sqlite3_errmsg(db)).to_string_lossy());
      }
    }
  }
}

impl SqliteLog {
  fn new() -> Self {
    unsafe {
      let mut db: *mut ffi::sqlite3 = std::ptr::null_mut();
      let filename = CString::new("logs.db").unwrap();

      let rc = ffi::sqlite3_open(filename.as_ptr(), &mut db);
      if rc != ffi::SQLITE_OK {
        panic!("open: {}", std::ffi::CStr::from_ptr(ffi::sqlite3_errmsg(db)).to_string_lossy());
      }

      let sql = CString::new(
        "CREATE TABLE IF NOT EXISTS logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts INTEGER,
            level INTEGER,
            log TEXT NOT NULL
        )",
      )
      .unwrap();

      let rc =
        ffi::sqlite3_exec(db, sql.as_ptr(), None, std::ptr::null_mut(), std::ptr::null_mut());
      if rc != ffi::SQLITE_OK {
        panic!("exec: {}", std::ffi::CStr::from_ptr(ffi::sqlite3_errmsg(db)).to_string_lossy());
      }

      Self { db: Mutex::new(db) }
    }
  }
}

struct LogEntity {
  message: String,
  level: Level,
  unix_time: u64,
}

fn main() {
  let sl = SqliteLog::new();

  trivial_log::builder()
    .default_format(|builder| {
      builder.appender_range(Level::Trace, Level::Error, |msg: &String| println!("{msg}"))
    })
    .format(
      |now, record| {
        Some(LogEntity {
          message: record.args().to_string(),
          level: record.level(),
          unix_time: now
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(std::time::Duration::new(0, 0))
            .as_secs(),
        })
      },
      |builder| builder.appender(Level::Error, sl),
    )
    .init()
    .unwrap();

  error!("critical error");
  println!("normal println");
  debug!("warning");
  let t = std::thread::spawn(move || {
    info!("猫 playing in your threads");
  });
  t.join().unwrap();

  trivial_log::free();
}

#[test]
fn run() {
  main();
}
