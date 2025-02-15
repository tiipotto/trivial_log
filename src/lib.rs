#![allow(clippy::type_complexity)] // TODO

use log::{Level, LevelFilter, Log, Metadata, Record};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::{Arc, Mutex, OnceLock, RwLock, RwLockReadGuard, TryLockError};
use std::time::{SystemTime, UNIX_EPOCH};

fn get_idx_for_level(level: Level) -> usize {
  match level {
    Level::Trace => 0,
    Level::Debug => 1,
    Level::Info => 2,
    Level::Warn => 3,
    Level::Error => 4,
  }
}

fn set_logger(level: LevelFilter) -> bool {
  static INIT: OnceLock<bool> = OnceLock::new();
  if *INIT.get_or_init(|| log::set_logger(&TL).is_ok()) {
    log::set_max_level(level);
    return true;
  }
  false
}

fn default_format(record: &Record<'_>) -> Option<String> {
  let prefix = match record.metadata().level() {
    Level::Error => "[E]",
    Level::Warn => "[W]",
    Level::Info => "[I]",
    Level::Debug => "[D]",
    Level::Trace => "[T]",
  };

  let mut buf = String::with_capacity(128);
  let instant = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
  //TODO better date+time

  use std::fmt::Write;
  if writeln!(buf, "{} {} {:?} {}", prefix, instant, std::thread::current().id(), record.args())
    .is_ok()
  {
    return Some(buf);
  }

  None
}

/// Will forward all log to stdout using the default format
pub fn init_stdout(level: LevelFilter) -> bool {
  builder().appender_filter(level, |msg: &str| println!("{}", msg)).init()
}

/// Will forward all log to stderr using the default format
pub fn init_stderr(level: LevelFilter) -> bool {
  builder().appender_filter(level, |msg: &str| eprintln!("{}", msg)).init()
}

/// Will forward all warn and below to stdout and all error to stderr
pub fn init_std(level: LevelFilter) -> bool {
  match level {
    LevelFilter::Off => builder().init(),
    LevelFilter::Error => builder().appender(Level::Error, |msg: &str| eprintln!("{}", msg)).init(),
    LevelFilter::Warn => builder()
      .appender(Level::Warn, |msg: &str| eprintln!("{}", msg))
      .appender(Level::Error, |msg: &str| eprintln!("{}", msg))
      .init(),
    LevelFilter::Info => builder()
      .appender_range(Level::Info, Level::Warn, |msg: &str| eprintln!("{}", msg))
      .appender(Level::Error, |msg: &str| eprintln!("{}", msg))
      .init(),
    LevelFilter::Debug => builder()
      .appender_range(Level::Debug, Level::Warn, |msg: &str| eprintln!("{}", msg))
      .appender(Level::Error, |msg: &str| eprintln!("{}", msg))
      .init(),
    LevelFilter::Trace => builder()
      .appender_range(Level::Trace, Level::Warn, |msg: &str| eprintln!("{}", msg))
      .appender(Level::Error, |msg: &str| eprintln!("{}", msg))
      .init(),
  }
}

pub fn builder() -> Builder {
  Builder::default()
}

pub struct Builder {
  format: Box<dyn Fn(&Record<'_>) -> Option<String> + Send + Sync>,
  appender: [Vec<Arc<dyn Appender>>; 5],
}

impl Default for Builder {
  fn default() -> Self {
    Self { format: Box::new(default_format), appender: [const { Vec::new() }; 5] }
  }
}

impl Builder {
  #[must_use]
  pub fn format(
    mut self,
    format: impl Fn(&Record<'_>) -> Option<String> + Send + Sync + 'static,
  ) -> Self {
    self.format = Box::new(format);
    self
  }

  #[must_use]
  pub fn appender(self, level: Level, appender: impl IntoAppender) -> Self {
    self.appender_range(level, level, appender)
  }

  #[must_use]
  pub fn appender_filter(self, filter: LevelFilter, appender: impl IntoAppender) -> Self {
    match filter {
      LevelFilter::Off => self,
      LevelFilter::Error => self.appender(Level::Error, appender),
      LevelFilter::Warn => self.appender_range(Level::Warn, Level::Error, appender),
      LevelFilter::Info => self.appender_range(Level::Info, Level::Error, appender),
      LevelFilter::Debug => self.appender_range(Level::Debug, Level::Error, appender),
      LevelFilter::Trace => self.appender_range(Level::Trace, Level::Error, appender),
    }
  }

  #[must_use]
  pub fn appender_range(mut self, from: Level, to: Level, appender: impl IntoAppender) -> Self {
    let wrap = appender.into_appender();
    for i in get_idx_for_level(from)..=get_idx_for_level(to) {
      self.appender[i].push(wrap.clone())
    }

    self
  }

  #[must_use]
  pub fn init(self) -> bool {
    let mut level = LevelFilter::Off;
    if !self.appender[4].is_empty() {
      level = LevelFilter::Error;
    }
    if !self.appender[3].is_empty() {
      level = LevelFilter::Warn;
    }
    if !self.appender[2].is_empty() {
      level = LevelFilter::Info;
    }
    if !self.appender[1].is_empty() {
      level = LevelFilter::Debug;
    }
    if !self.appender[0].is_empty() {
      level = LevelFilter::Trace;
    }

    let mut guard = TL.0.write().unwrap_or_else(|poison| {
      TL.0.clear_poison();
      poison.into_inner()
    });

    _ = guard.take();

    if !set_logger(level) {
      return false;
    }

    if level == LevelFilter::Off {
      return true;
    }

    *guard = Some(TrivialLogInner { format: self.format, appender: self.appender });

    true
  }
}

pub fn free() {
  TL.0
    .write()
    .unwrap_or_else(|poison| {
      TL.0.clear_poison();
      poison.into_inner()
    })
    .take();
}

static TL: TrivialLog = TrivialLog(RwLock::new(None));

pub trait Appender: Send + Sync {
  fn append_log_message(&self, message: &str);
}

pub trait IntoAppender {
  fn into_appender(self) -> Arc<dyn Appender>;
}

impl<T> IntoAppender for T
where
  T: Appender + 'static,
{
  fn into_appender(self) -> Arc<dyn Appender> {
    Arc::new(self)
  }
}

impl<T> Appender for T
where
  T: Fn(&str) + Send + Sync,
{
  fn append_log_message(&self, message: &str) {
    self(message)
  }
}

struct AppenderWriter<X: Write + Send>(Mutex<BufWriter<X>>);

impl<X: Write + Send> Appender for AppenderWriter<X> {
  fn append_log_message(&self, message: &str) {
    if let Ok(mut guard) = self.0.lock() {
      //We ignore errors
      _ = guard.write_all(message.as_bytes());
      _ = guard.write_all([b'\n'].as_slice()); //We dont do CRLF here windows and mac can suck dick
      _ = guard.flush();
    }
  }
}

impl<X: Write + Send + 'static> IntoAppender for BufWriter<X> {
  fn into_appender(self) -> Arc<dyn Appender> {
    Arc::new(AppenderWriter(Mutex::new(self)))
  }
}

impl IntoAppender for File {
  fn into_appender(self) -> Arc<dyn Appender> {
    Arc::new(AppenderWriter(Mutex::new(BufWriter::new(self))))
  }
}

struct TrivialLogInner {
  format: Box<dyn Fn(&Record<'_>) -> Option<String> + Send + Sync>,
  appender: [Vec<Arc<dyn Appender>>; 5], //5 is number of levels in log crate
}

impl TrivialLogInner {
  fn is_enabled(&self, level: Level) -> bool {
    !&self.appender[get_idx_for_level(level)].is_empty()
  }
}

struct TrivialLog(RwLock<Option<TrivialLogInner>>);
impl Log for TrivialLog {
  fn enabled(&self, metadata: &Metadata<'_>) -> bool {
    if let Some(guard) = self.guard() {
      return guard.as_ref().map(|inner| inner.is_enabled(metadata.level())).unwrap_or(false);
    }
    false
  }

  fn log(&self, record: &Record<'_>) {
    if let Some(guard) = self.guard() {
      if let Some(inner) = guard.as_ref() {
        let appender_list = &inner.appender[get_idx_for_level(record.level())];
        if appender_list.is_empty() {
          return;
        }

        if let Some(fmt) = (inner.format)(record) {
          for appender in appender_list {
            appender.append_log_message(fmt.as_str());
          }
        }
      };
    }
  }

  fn flush(&self) {}
}

impl TrivialLog {
  fn guard(&self) -> Option<RwLockReadGuard<'_, Option<TrivialLogInner>>> {
    match self.0.try_read() {
      Ok(guard) => Some(guard),
      Err(TryLockError::Poisoned(poison)) => {
        self.0.clear_poison();
        Some(poison.into_inner())
      }
      Err(TryLockError::WouldBlock) => None, //Logger is currently being configured in another thread.
    }
  }
}
