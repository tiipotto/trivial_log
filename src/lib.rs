#![expect(clippy::type_complexity)] // TODO
#![expect(clippy::result_unit_err)] // TODO, we shouldn't really need an error type for log already initialized

use log::{Level, LevelFilter, Log, Metadata, Record};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::{mpsc, Arc, Mutex, OnceLock, RwLock, RwLockReadGuard, TryLockError};
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

fn set_logger(level: LevelFilter) -> Result<(), ()> {
  //Purpose of this once look is to track if we are the logging impl in use or not.
  //We only have to call set_logger once, as everything except the first call will always fail.
  static INIT: OnceLock<bool> = OnceLock::new();

  if *INIT.get_or_init(|| log::set_logger(&TL).is_ok()) {
    //We cant do this inside the init fn, or we cannot "change" the level later on anymore!
    log::set_max_level(level);
    return Ok(());
  }

  Err(())
}

fn get_level_for_handlers(handlers: &Vec<Box<dyn Handler>>) -> LevelFilter {
  let mut level = LevelFilter::Off;
  for handler in handlers {
    match level {
      LevelFilter::Off => {
        if handler.is_enabled(Level::Trace) {
          return LevelFilter::Trace;
        }
        if handler.is_enabled(Level::Debug) {
          level = LevelFilter::Debug;
          continue;
        }
        if handler.is_enabled(Level::Info) {
          level = LevelFilter::Info;
          continue;
        }
        if handler.is_enabled(Level::Warn) {
          level = LevelFilter::Warn;
          continue;
        }
        if handler.is_enabled(Level::Error) {
          level = LevelFilter::Error;
          continue;
        }
      }
      LevelFilter::Error => {
        if handler.is_enabled(Level::Trace) {
          return LevelFilter::Trace;
        }
        if handler.is_enabled(Level::Debug) {
          level = LevelFilter::Debug;
          continue;
        }
        if handler.is_enabled(Level::Info) {
          level = LevelFilter::Info;
          continue;
        }
        if handler.is_enabled(Level::Warn) {
          level = LevelFilter::Warn;
          continue;
        }
      }
      LevelFilter::Warn => {
        if handler.is_enabled(Level::Trace) {
          return LevelFilter::Trace;
        }
        if handler.is_enabled(Level::Debug) {
          level = LevelFilter::Debug;
          continue;
        }
        if handler.is_enabled(Level::Info) {
          level = LevelFilter::Info;
          continue;
        }
      }
      LevelFilter::Info => {
        if handler.is_enabled(Level::Trace) {
          return LevelFilter::Trace;
        }
        if handler.is_enabled(Level::Debug) {
          level = LevelFilter::Debug;
          continue;
        }
      }
      LevelFilter::Debug => {
        if handler.is_enabled(Level::Trace) {
          return LevelFilter::Trace;
        }
      }
      LevelFilter::Trace => unreachable!(),
    }
  }
  level
}

fn default_format(now: SystemTime, record: &Record<'_>) -> Option<String> {
  let prefix = match record.metadata().level() {
    Level::Error => "[E]",
    Level::Warn => "[W]",
    Level::Info => "[I]",
    Level::Debug => "[D]",
    Level::Trace => "[T]",
  };

  let mut buf = String::with_capacity(128);
  let instant = now.duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
  #[cfg(feature = "chrono")]
  let instant = {
    const FORMAT: chrono::format::StrftimeItems<'_> = chrono::format::StrftimeItems::new("%d.%m.%Y %H:%M:%S%.3f UTC");
    chrono::DateTime::from_timestamp_millis(instant as i64).map(|dt| dt.format_with_items(FORMAT)).unwrap()
  };

  use std::fmt::Write;
  if writeln!(buf, "{} - {} - {:?} - {}", prefix, instant, std::thread::current().id(), record.args())
    .is_ok()
  {
    return Some(buf);
  }

  None
}

/// Will forward all log to stdout using the default format
pub fn init_stdout(level: LevelFilter) -> Result<(), ()> {
  builder().appender_filter(level, |msg: &String| print!("{}", msg)).init()
}

/// Will forward all log to stderr using the default format
pub fn init_stderr(level: LevelFilter) -> Result<(), ()> {
  builder().appender_filter(level, |msg: &String| eprint!("{}", msg)).init()
}

/// Will forward all warn and below to stdout and all error to stderr
pub fn init_std(level: LevelFilter) -> Result<(), ()> {
  match level {
    LevelFilter::Off => builder().init(),
    LevelFilter::Error => {
      builder().appender(Level::Error, |msg: &String| eprint!("{}", msg)).init()
    }
    LevelFilter::Warn => builder()
      .appender(Level::Warn, |msg: &String| eprint!("{}", msg))
      .appender(Level::Error, |msg: &String| eprint!("{}", msg))
      .init(),
    LevelFilter::Info => builder()
      .appender_range(Level::Info, Level::Warn, |msg: &String| eprint!("{}", msg))
      .appender(Level::Error, |msg: &String| eprint!("{}", msg))
      .init(),
    LevelFilter::Debug => builder()
      .appender_range(Level::Debug, Level::Warn, |msg: &String| eprint!("{}", msg))
      .appender(Level::Error, |msg: &String| eprint!("{}", msg))
      .init(),
    LevelFilter::Trace => builder()
      .appender_range(Level::Trace, Level::Warn, |msg: &String| eprint!("{}", msg))
      .appender(Level::Error, |msg: &String| eprint!("{}", msg))
      .init(),
  }
}

#[must_use]
pub fn builder() -> Builder<String> {
  Builder::default()
}

pub struct Builder<T> {
  handlers: Vec<Box<dyn Handler>>,
  format: Box<dyn Fn(SystemTime, &Record<'_>) -> Option<T> + Send + Sync>,
  appender: [Vec<Arc<dyn Appender<T>>>; 5],
}

impl Default for Builder<String> {
  fn default() -> Self {
    Self {
      handlers: Vec::new(),
      format: Box::new(default_format),
      appender: [const { Vec::new() }; 5],
    }
  }
}

impl<T: 'static> Builder<T> {
  fn finish_handler(mut self) -> Vec<Box<dyn Handler>> {
    let mut is_empty = true;
    for n in &self.appender {
      if !n.is_empty() {
        is_empty = false;
        break;
      }
    }

    if is_empty {
      return self.handlers;
    }

    self.handlers.push(Box::new(HandlerImpl { format: self.format, appender: self.appender }));

    self.handlers
  }

  #[must_use]
  pub fn format<Y>(
    self,
    format: impl Fn(SystemTime, &Record<'_>) -> Option<Y> + Send + Sync + 'static,
  ) -> Builder<Y> {
    let handlers = self.finish_handler();
    let format = Box::new(format);
    Builder { handlers, format, appender: [const { Vec::new() }; 5] }
  }

  #[must_use]
  pub fn appender(self, level: Level, appender: impl IntoAppender<T>) -> Self {
    self.appender_range(level, level, appender)
  }

  #[must_use]
  pub fn appender_filter(self, filter: LevelFilter, appender: impl IntoAppender<T>) -> Self {
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
  pub fn appender_range(mut self, from: Level, to: Level, appender: impl IntoAppender<T>) -> Self {
    let wrap = appender.into_appender();
    for i in get_idx_for_level(from)..=get_idx_for_level(to) {
      if let Some(a) = self.appender.get_mut(i) {
        a.push(wrap.clone())
      }
    }

    self
  }

  pub fn init(self) -> Result<(), ()> {
    let handlers = self.finish_handler();

    let level = get_level_for_handlers(&handlers);

    let mut guard = TL.0.write().unwrap_or_else(|poison| {
      TL.0.clear_poison();
      poison.into_inner()
    });

    _ = guard.take();
    set_logger(level)?;

    if level == LevelFilter::Off {
      return Ok(());
    }

    *guard = Some(HandlerCompound::new(handlers));

    Ok(())
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

trait Handler: Sync + Send {
  fn log(&self, now: SystemTime, record: &Record<'_>);

  fn is_enabled(&self, level: Level) -> bool;
}

struct HandlerImpl<T> {
  format: Box<dyn Fn(SystemTime, &Record<'_>) -> Option<T> + Send + Sync>,
  appender: [Vec<Arc<dyn Appender<T>>>; 5], //5 is number of levels in log crate
}

impl<T> Handler for HandlerImpl<T> {
  fn log(&self, now: SystemTime, record: &Record<'_>) {
    let Some(appender_list) = self.appender.get(get_idx_for_level(record.level())) else {
      unreachable!();
    };

    if appender_list.is_empty() {
      return;
    }

    if let Some(fmt) = (self.format)(now, record) {
      for appender in appender_list {
        appender.append_log_message(&fmt);
      }
    }
  }

  fn is_enabled(&self, level: Level) -> bool {
    let Some(a) = self.appender.get(get_idx_for_level(level)) else { return false };
    !a.is_empty()
  }
}

pub trait Appender<T>: Send + Sync {
  fn append_log_message(&self, message: &T);
}

pub trait IntoAppender<T> {
  fn into_appender(self) -> Arc<dyn Appender<T>>;
}

struct HandlerCompound {
  handlers: Vec<Box<dyn Handler>>,
  handler_indices: [Vec<usize>; 5],
}

impl HandlerCompound {
  fn new(handlers: Vec<Box<dyn Handler>>) -> Self {
    let mut handler_indices: [Vec<usize>; 5] = [const { Vec::new() }; 5];

    for (idx, handler) in handlers.iter().enumerate() {
      if handler.is_enabled(Level::Trace) {
        handler_indices[0].push(idx);
      }
      if handler.is_enabled(Level::Debug) {
        handler_indices[1].push(idx);
      }
      if handler.is_enabled(Level::Info) {
        handler_indices[2].push(idx);
      }
      if handler.is_enabled(Level::Warn) {
        handler_indices[3].push(idx);
      }
      if handler.is_enabled(Level::Error) {
        handler_indices[4].push(idx);
      }
    }

    Self { handlers, handler_indices }
  }
  fn is_enabled(&self, level: Level) -> bool {
    self.handler_indices.get(get_idx_for_level(level)).map(Vec::is_empty).unwrap_or(false)
  }
  fn log(&self, record: &Record<'_>) {
    let now: SystemTime = SystemTime::now();
    if let Some(indices) = self.handler_indices.get(get_idx_for_level(record.level())) {
      for idx in indices {
        if let Some(handler) = self.handlers.get(*idx) {
          handler.log(now, record)
        }
      }
    }

  }
}

struct TrivialLog(RwLock<Option<HandlerCompound>>);
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
        inner.log(record);
      }
    }
  }

  fn flush(&self) {}
}

impl TrivialLog {
  fn guard(&self) -> Option<RwLockReadGuard<'_, Option<HandlerCompound>>> {
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

impl<T, Y> IntoAppender<Y> for T
where
  T: Appender<Y> + 'static,
{
  fn into_appender(self) -> Arc<dyn Appender<Y>> {
    Arc::new(self)
  }
}

impl<T, Y> Appender<Y> for T
where
  T: Fn(&Y) + Send + Sync,
{
  fn append_log_message(&self, message: &Y) {
    self(message)
  }
}

struct AppenderWriter<X: Write + Send>(Mutex<BufWriter<X>>);

impl<X: Write + Send> Appender<String> for AppenderWriter<X> {
  fn append_log_message(&self, message: &String) {
    if let Ok(mut guard) = self.0.lock() {
      //We ignore errors
      _ = guard.write_all(message.as_bytes());
      _ = guard.flush();
    }
  }
}

impl<X: Write + Send + 'static> IntoAppender<String> for BufWriter<X> {
  fn into_appender(self) -> Arc<dyn Appender<String>> {
    Arc::new(AppenderWriter(Mutex::new(self)))
  }
}

impl IntoAppender<String> for File {
  fn into_appender(self) -> Arc<dyn Appender<String>> {
    Arc::new(AppenderWriter(Mutex::new(BufWriter::new(self))))
  }
}

impl IntoAppender<String> for &Path {
  fn into_appender(self) -> Arc<dyn Appender<String>> {
    match OpenOptions::new().append(true).create(true).open(self) {
      Ok(file) => file.into_appender(),
      Err(err) => {
        panic!("Failed to open or create log file {} reason: {}", self.to_string_lossy(), err)
      }
    }
  }
}

impl<X: Send + Clone> Appender<X> for mpsc::Sender<X> {
  fn append_log_message(&self, message: &X) {
    //We don't care if the receiving end is dead.
    _ = self.send(message.clone());
  }
}

impl<X: Send + Clone> Appender<X> for mpsc::SyncSender<X> {
  fn append_log_message(&self, message: &X) {
    //We don't care if the receiving end is dead.
    _ = self.send(message.clone());
  }
}
