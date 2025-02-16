#![expect(clippy::result_unit_err)] // TODO, we shouldn't really need an error type for log already initialized

use log::{Level, LevelFilter, Log, Metadata, Record};
use std::sync::{Arc, OnceLock, RwLock, RwLockReadGuard, TryLockError};
use std::time::SystemTime;

mod impls;
mod util;

fn set_logger(level: LevelFilter) -> Result<(), ()> {
  //Purpose of this once look is to track if we are the logging impl in use or not.
  //We only have to call log::set_logger once, as everything except the first call will always fail.
  static INIT: OnceLock<bool> = OnceLock::new();

  if *INIT.get_or_init(|| log::set_logger(&TL).is_ok()) {
    //We cant do this inside the init fn, or we cannot "change" the level later on anymore!
    log::set_max_level(level);
    return Ok(());
  }

  Err(())
}

/// Initializes `log` to forward all log to stdout using the default format
pub fn init_stdout(level: LevelFilter) -> Result<(), ()> {
  builder()
    .default_format(|builder| builder.appender_filter(level, |msg: &String| print!("{}", msg)))
    .init()
}

/// Initializes `log` to forward all log to stderr using the default format
pub fn init_stderr(level: LevelFilter) -> Result<(), ()> {
  builder()
    .default_format(|builder| builder.appender_filter(level, |msg: &String| eprint!("{}", msg)))
    .init()
}

/// Initializes `log` to forward all warn and below to stdout and all error to stderr
pub fn init_std(level: LevelFilter) -> Result<(), ()> {
  match level {
    LevelFilter::Off => builder().init(),
    LevelFilter::Error => builder()
      .default_format(|builder| builder.appender(Level::Error, |msg: &String| eprint!("{}", msg)))
      .init(),
    LevelFilter::Warn => builder()
      .default_format(|builder| {
        builder
          .appender(Level::Warn, |msg: &String| eprint!("{}", msg))
          .appender(Level::Error, |msg: &String| eprint!("{}", msg))
      })
      .init(),
    LevelFilter::Info => builder()
      .default_format(|builder| {
        builder
          .appender_range(Level::Info, Level::Warn, |msg: &String| eprint!("{}", msg))
          .appender(Level::Error, |msg: &String| eprint!("{}", msg))
      })
      .init(),
    LevelFilter::Debug => builder()
      .default_format(|builder| {
        builder
          .appender_range(Level::Debug, Level::Warn, |msg: &String| eprint!("{}", msg))
          .appender(Level::Error, |msg: &String| eprint!("{}", msg))
      })
      .init(),
    LevelFilter::Trace => builder()
      .default_format(|fmt| {
        fmt
          .appender_range(Level::Trace, Level::Warn, |msg: &String| eprint!("{}", msg))
          .appender(Level::Error, |msg: &String| eprint!("{}", msg))
      })
      .init(),
  }
}

#[must_use]
pub fn builder() -> Builder {
  Builder::default()
}

pub struct AppenderBuilder<T> {
  format: Box<dyn Fn(SystemTime, &Record<'_>) -> Option<T> + Send + Sync>,
  appender: [Vec<Arc<dyn Appender<T>>>; 5],
}

impl<T> AppenderBuilder<T> {
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
    for i in util::get_idx_for_level(from)..=util::get_idx_for_level(to) {
      if let Some(a) = self.appender.get_mut(i) {
        a.push(wrap.clone())
      }
    }

    self
  }
}

#[derive(Default)]
pub struct Builder {
  handlers: Vec<Box<dyn Handler>>,
}

impl Builder {
  #[must_use]
  pub fn default_format(
    self,
    functor: impl FnOnce(AppenderBuilder<String>) -> AppenderBuilder<String>,
  ) -> Self {
    self.format(util::default_format, functor)
  }

  #[must_use]
  pub fn format<Y: 'static>(
    mut self,
    format: impl Fn(SystemTime, &Record<'_>) -> Option<Y> + Send + Sync + 'static,
    functor: impl FnOnce(AppenderBuilder<Y>) -> AppenderBuilder<Y>,
  ) -> Self {
    let result =
      functor(AppenderBuilder { format: Box::new(format), appender: [const { Vec::new() }; 5] });
    let mut is_empty = true;
    for n in &result.appender {
      if !n.is_empty() {
        is_empty = false;
        break;
      }
    }

    if is_empty {
      return self;
    }

    self.handlers.push(Box::new(HandlerImpl { format: result.format, appender: result.appender }));
    self
  }

  pub fn init(self) -> Result<(), ()> {
    let level = util::get_level_for_handlers(&self.handlers);

    let mut guard = TL.0.write().unwrap_or_else(|poison| {
      TL.0.clear_poison();
      poison.into_inner()
    });

    _ = guard.take();
    set_logger(level)?;

    if level == LevelFilter::Off {
      return Ok(());
    }

    *guard = Some(HandlerCompound::new(self.handlers));

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

static TL: LogImpl = LogImpl(RwLock::new(None));

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
    let Some(appender_list) = self.appender.get(util::get_idx_for_level(record.level())) else {
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
    let Some(a) = self.appender.get(util::get_idx_for_level(level)) else { return false };
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
    self.handler_indices.get(util::get_idx_for_level(level)).map(Vec::is_empty).unwrap_or(false)
  }
  fn log(&self, record: &Record<'_>) {
    let now: SystemTime = SystemTime::now();
    if let Some(indices) = self.handler_indices.get(util::get_idx_for_level(record.level())) {
      for idx in indices {
        if let Some(handler) = self.handlers.get(*idx) {
          handler.log(now, record)
        }
      }
    }
  }
}

struct LogImpl(RwLock<Option<HandlerCompound>>);
impl Log for LogImpl {
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

impl LogImpl {
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
