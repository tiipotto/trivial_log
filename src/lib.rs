//!
//!# `trivial_log`
//! This is intended to be a no-bloat implementation for [log](https://github.com/rust-lang/log).
//! It includes simple defaults while still providing good flexibility for more advanced use cases.
//!
//!# Example
//!```rust
//!  trivial_log::init_std(log::LevelFilter::Trace).unwrap();
//!  log::error!("An error has occurred, please help!");
//!```
//!

use log::{Level, LevelFilter, Log, Metadata, Record};
use std::sync::{Arc, RwLock, RwLockReadGuard, TryLockError};
use std::time::SystemTime;

/// error types
mod error;

/// Implementations for `FormatFn` or `Appender`/`IntoAppender` for various types in the standard library.
mod impls;

/// Utility functions
mod util;

pub use error::Error;

/// Initializes `log` to forward all log to stdout using the default format
/// # Errors
/// Only if there is already another log implementation initialized
pub fn init_stdout(level: LevelFilter) -> Result<(), Error> {
  builder()
    .default_format(|builder| builder.appender_filter(level, |msg: &String| print!("{msg}")))
    .init()
}

/// Initializes `log` to forward all log to stderr using the default format
/// # Errors
/// Only if there is already another log implementation initialized
pub fn init_stderr(level: LevelFilter) -> Result<(), Error> {
  builder()
    .default_format(|builder| builder.appender_filter(level, |msg: &String| eprint!("{msg}")))
    .init()
}

/// Initializes `log` to forward all warn and below to stdout and all error to stderr
/// # Errors
/// Only if there is already another log implementation initialized
pub fn init_std(level: LevelFilter) -> Result<(), Error> {
  match level {
    LevelFilter::Off => builder().init(),
    LevelFilter::Error => builder()
      .default_format(|builder| builder.appender(Level::Error, |msg: &String| eprint!("{msg}")))
      .init(),
    LevelFilter::Warn => builder()
      .default_format(|builder| {
        builder
          .appender(Level::Warn, |msg: &String| eprint!("{msg}"))
          .appender(Level::Error, |msg: &String| eprint!("{msg}"))
      })
      .init(),
    LevelFilter::Info => builder()
      .default_format(|builder| {
        builder
          .appender_range(Level::Info, Level::Warn, |msg: &String| eprint!("{msg}"))
          .appender(Level::Error, |msg: &String| eprint!("{msg}"))
      })
      .init(),
    LevelFilter::Debug => builder()
      .default_format(|builder| {
        builder
          .appender_range(Level::Debug, Level::Warn, |msg: &String| eprint!("{msg}"))
          .appender(Level::Error, |msg: &String| eprint!("{msg}"))
      })
      .init(),
    LevelFilter::Trace => builder()
      .default_format(|fmt| {
        fmt
          .appender_range(Level::Trace, Level::Warn, |msg: &String| eprint!("{msg}"))
          .appender(Level::Error, |msg: &String| eprint!("{msg}"))
      })
      .init(),
  }
}

#[must_use]
pub fn builder() -> Builder {
  Builder::default()
}

/// Builder for adding appenders to a format `Fn`.
pub struct AppenderBuilder<T> {
  /// The format fn
  format: Box<FormatFn<T>>,
  /// The appenders grouped by level.
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
        a.push(Arc::clone(&wrap));
      }
    }

    self
  }
}

/// Builder for configuring `trivial_log`.
/// Use `trivial_log::builder()` to obtain a new instance of this struct.
#[derive(Default)]
pub struct Builder {
  /// All handlers already built in the builder. init will transform this into a `HandlerCompound`
  handlers: Vec<Box<dyn Handler>>,
}

impl Builder {
  /// Use the default format for some appenders.
  /// The passed builder argument `FnOnce` can be used to register the appenders.
  ///
  /// Note: It will lead to better performance if all appenders that use the same format are grouped together and registered in the same closure!
  #[must_use]
  pub fn default_format(
    self,
    builder: impl FnOnce(AppenderBuilder<String>) -> AppenderBuilder<String>,
  ) -> Self {
    self.format(util::default_format, builder)
  }

  /// Use a provided format for some appenders.
  /// The passed format argument `Fn` will provide the format struct. (for example a String)
  /// The passed builder argument `FnOnce` can be used to register the appenders which will consume the format struct.
  ///
  /// Note: It will lead to better performance if all appenders that use the same format are grouped together and registered in the same closure!
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

  /// Initialize the logging implementation
  /// # Errors
  /// Only if a different logger implementation is in use.
  /// If this fn errors then it was essentially a noop.
  pub fn init(self) -> Result<(), Error> {
    let level = util::get_level_for_handlers(&self.handlers);

    let mut guard = TL.0.write().unwrap_or_else(|poison| {
      TL.0.clear_poison();
      poison.into_inner()
    });

    _ = guard.take();
    util::set_log_logger_impl_and_level(level)?;

    if level == LevelFilter::Off {
      return Ok(());
    }

    *guard = Some(HandlerCompound::new(self.handlers));
    drop(guard);

    Ok(())
  }
}

/// This fn will remove all formats and appenders from the log impl making it essentially a noop.
///
///
/// This fn is always fully safe to call.
/// This fn will block until all appenders are finished writing concurrent ongoing messages.
///
/// initializing a new logger using the normal Builder is always possible after this fn has been called.
///
/// There is no need to call this function before exiting the program but doing so will cause
/// some allocated memory to be freed and therefore will make valgrind and other leak checkers very happy,
/// Because this function leaves behind a "noop" logger, there will NOT be a problem
/// if the program calls log! after this fn is called.
///
/// This function does nothing if called repeatedly.
/// This function does nothing if the actual logger implementation in use by the log crate is a different one.
///
/// Note: Calling this fn will not allow you to switch to a different logger implementation since that is not a supported use case of the log crate itself.
pub fn free() {
  TL.0
    .write()
    .unwrap_or_else(|poison| {
      TL.0.clear_poison();
      poison.into_inner()
    })
    .take();
}

/// The static state holder
static TL: LogImpl = LogImpl(RwLock::new(None));

/// Trait to hide the static dispatch type T from the rest of the implementation behind dynamic dispatch.
trait Handler: Sync + Send {
  ///Log the record for the given time.
  fn log(&self, now: SystemTime, record: &Record<'_>);

  /// Does the handler have any appenders for the given level?
  fn is_enabled(&self, level: Level) -> bool;
}

/// The format fn
type FormatFn<T> = dyn Fn(SystemTime, &Record<'_>) -> Option<T> + Send + Sync;

/// Contains a format fn as well as all appenders associated with the format fn.
struct HandlerImpl<T> {
  /// The format fn to use to format the `log::Record`
  format: Box<FormatFn<T>>,
  /// The appenders for each level
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

/// This trait defines an appender which consumes a formatted log message of type T and will "write" it to somewhere like stdout/disk/network/...
pub trait Appender<T>: Send + Sync {
  /// Called for each formatted log message.
  fn append_log_message(&self, message: &T);
}

/// Custom "Into" trait that produces an Arc<dyn `Appender<T>`.
/// This trait does not usually need to implemented manually.
/// It is implemented for all Appender structs by default.
pub trait IntoAppender<T> {
  fn into_appender(self) -> Arc<dyn Appender<T>>;
}

/// Contains a Vec of handlers and also groups them by level.
struct HandlerCompound {
  /// Contains all handlers
  handlers: Vec<Box<dyn Handler>>,
  /// contains indices into handlers vec for each level.
  handler_indices: [Vec<usize>; 5],
}

impl HandlerCompound {
  /// Pre-calculates which handlers handle which levels and optimizes the Vec for later use.
  fn new(mut handlers: Vec<Box<dyn Handler>>) -> Self {
    handlers.shrink_to_fit();
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

    handler_indices.iter_mut().filter(|idx_vec| !idx_vec.is_empty()).for_each(Vec::shrink_to_fit);

    Self { handlers, handler_indices }
  }

  /// Returns true if at least one handler can handle the level
  fn is_enabled(&self, level: Level) -> bool {
    self.handler_indices.get(util::get_idx_for_level(level)).is_some_and(Vec::is_empty)
  }

  /// Delegates to the correct handlers for the given log levels
  fn log(&self, record: &Record<'_>) {
    let now: SystemTime = SystemTime::now();
    if let Some(indices) = self.handler_indices.get(util::get_idx_for_level(record.level())) {
      for idx in indices {
        if let Some(handler) = self.handlers.get(*idx) {
          handler.log(now, record);
        }
      }
    }
  }
}

/// Private static state that holds some heap allocated objects if initialized or nothing if not.
struct LogImpl(RwLock<Option<HandlerCompound>>);
impl Log for LogImpl {
  fn enabled(&self, metadata: &Metadata<'_>) -> bool {
    if let Some(guard) = self.guard() {
      return guard.as_ref().is_some_and(|inner| inner.is_enabled(metadata.level()));
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
  /// Returns a shared read guard of the static state
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
