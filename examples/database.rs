use std::sync::Mutex;

use log::{debug, error, info, Level};
use rusqlite::Connection;
use trivial_log::Appender;

#[derive(Debug)]
struct SqliteLog {
  conn: Mutex<Connection>,
}

impl Appender<LogEntity> for SqliteLog {
  fn append_log_message(&self, le: &LogEntity) {
    self
      .conn
      .lock()
      .unwrap()
      .execute(
        "INSERT INTO logs (ts, level, log) VALUES (?, ?, ?)",
        (
          i64::try_from(le.unix_time).unwrap_or(0),
          i64::try_from(le.level as usize).unwrap_or(1),
          &le.message,
        ),
      )
      .unwrap();
  }
}

impl SqliteLog {
  fn new() -> Self {
    let conn = Connection::open("logs.db").unwrap();
    conn
      .execute(
        "CREATE TABLE IF NOT EXISTS logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts INTEGER,
            level INTEGER,
            log TEXT NOT NULL
        )",
        (),
      )
      .unwrap();

    Self { conn: Mutex::new(conn) }
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
