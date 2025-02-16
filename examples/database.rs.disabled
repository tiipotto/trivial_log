use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use log::{debug, error, info, Level};
use rusqlite::Connection;
use trivial_log::Appender;

#[derive(Debug)]
struct SqliteLog {
  conn: Mutex<Connection>,
}

impl Appender for SqliteLog {
  fn append_log_message(&self, message: &str) {
    let unix_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    self
      .conn
      .lock()
      .unwrap()
      .execute("INSERT INTO logs (ts, log) VALUES (?, ?)", (unix_time, message))
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
            ts,
            log TEXT NOT NULL
        )",
        (),
      )
      .unwrap();

    Self { conn: Mutex::new(conn) }
  }
}

fn main() {
  let sl = SqliteLog::new();

  trivial_log::builder()
    .appender(Level::Error, sl)
    .appender_range(Level::Trace, Level::Error, |msg: &str| println!("{}", msg))
    .init()
    .unwrap();

  error!("fuck you");
  println!("normal println");
  debug!("warning");
  let t = std::thread::spawn(move || {
    info!("猫");
  });
  t.join().unwrap();

  trivial_log::free();
}

#[test]
fn run() {
  main();
}
