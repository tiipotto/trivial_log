use std::fs::File;
use std::io::{stdout, Write};
use std::sync::Mutex;

use log::{debug, error, info, Level, LevelFilter, Record};
use trivial_log::{free, init, Logger, TrivialLog};

fn printer(buf: &mut Box<dyn Write + Send>, record: &Record<'_>) {
  let prefix = match record.metadata().level() {
    Level::Error => "[E]",
    Level::Info => "[I]",
    _ => "[Cat]",
  };

  writeln!(buf, "{} {:?} {}", prefix, std::thread::current().id(), record.args().to_string())
    .unwrap()
}

fn main() {
  let stdout_logger = Logger {
    level: LevelFilter::Info,
    writer: Mutex::new(Box::new(stdout())),
    format: Box::new(printer),
  };
  let errlog = Logger {
    level: LevelFilter::Error,
    writer: Mutex::new(Box::new(File::create("katze.log").unwrap())),
    format: Box::new(printer),
  };
  init(LevelFilter::Info, vec![stdout_logger, errlog]);

  error!("fuck you");
  println!("normal println");
  debug!("warning");
  let t = std::thread::spawn(move || {
    info!("猫");
  });
  t.join().unwrap();

  trivial_log::free();
}
