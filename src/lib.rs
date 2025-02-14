use std::io::Write;
use std::sync::Mutex;

use log::{LevelFilter, Log, Record};

struct Singleton {
  loggers: Vec<Logger>,
  level: LevelFilter,
}

pub struct Logger {
  pub level: LevelFilter,
  pub writer: Mutex<Box<dyn Write + Send>>,
  pub format: Box<dyn Fn(&mut Box<dyn Write + Send>, &Record<'_>) + Send>,
}

static SINGLETON: Mutex<Option<Singleton>> = Mutex::new(None);

impl Drop for TrivialLog {
  fn drop(&mut self) {
    *SINGLETON.lock().unwrap() = None;
  }
}

pub struct TrivialLog {}

impl Log for TrivialLog {
  fn enabled(&self, metadata: &log::Metadata) -> bool {
    metadata.level() <= self.level()
  }

  fn log(&self, record: &log::Record) {
    self.inner_log(record);
  }

  fn flush(&self) {}
}

impl TrivialLog {
  #[must_use]
  pub fn init(level: LevelFilter, loggers: Vec<Logger>) -> Self {
    let single = Singleton { level, loggers };

    *SINGLETON.lock().unwrap() = Some(single);

    let r = log::set_boxed_logger(Box::new(TrivialLog {}));
    if r.is_ok() {
      log::set_max_level(level);
    }

    Self {}
  }

  fn level(&self) -> LevelFilter {
    let wat = &*SINGLETON.lock().unwrap();
    match wat {
      Some(s) => s.level,
      None => LevelFilter::Error,
    }
  }

  fn inner_log(&self, record: &log::Record) {
    let wat = &mut *SINGLETON.lock().unwrap();
    if let Some(s) = wat {
      for l in s.loggers.iter_mut() {
        if record.metadata().level() > s.level || record.metadata().level() > l.level {
          continue;
        }
        let buf = &mut *l.writer.lock().unwrap();
        (l.format)(buf, record);
      }
    }
  }
}
