use log::{debug, error, info, Level};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Serialize, Deserialize)]
struct LogEntity {
  message: String,
  level: String,
  thread: String,
  timestamp: u128,
}

fn main() {
  trivial_log::builder()
    .default_format(|builder| {
      builder.appender(Level::Info, |info: &String| {
        // You can mix and match between different formats
        print!("NON JSON INFO: {info}");
      })
    })
    .format(
      |now, record| {
        Some(LogEntity {
          message: record.args().to_string(),
          level: record.level().to_string(),
          thread: format!("{:?}", std::thread::current().id()),
          timestamp: now.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis(),
        })
      },
      |builder| {
        builder
          .appender_range(Level::Trace, Level::Warn, |x: &LogEntity| {
            let json = serde_json::to_string(x).unwrap();
            //You would obviously send this to some remote endpoint or write this to a disk.
            println!("NON ERROR: {json}");
          })
          .appender(Level::Error, |x: &LogEntity| {
            let json = serde_json::to_string(x).unwrap();
            //You would obviously send this to some remote endpoint or write this to a disk.
            println!("ERROR ONLY: {json}");
          })
      },
    )
    .init()
    .ok();

  error!("An error has occurred, please help!");
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
