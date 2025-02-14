use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::Error;
use crate::{Handler, Level, LevelFilter, TL};
use log::Record;

/// Sets the `trivial_log` impl into the `log` crate or fails.
/// Also sets the log level if it succeeds or if a previous call to it already succeeded.
/// # Errors
/// If another log implementation is already loaded.
pub fn set_log_logger_impl_and_level(level: LevelFilter) -> Result<(), Error> {
  /// Purpose of this once look is to track if we are the logging impl in use or not.
  /// We only have to call `log::set_logger` once, as everything except the first call will always fail.
  static INIT: OnceLock<bool> = OnceLock::new();

  if *INIT.get_or_init(|| log::set_logger(&TL).is_ok()) {
    // We cant do this inside the init fn, or we cannot "change" the level later on anymore!
    log::set_max_level(level);
    return Ok(());
  }

  Err(Error::AlreadyInitialized)
}

/// transform `log::Level` into a numeric index
pub const fn get_idx_for_level(level: Level) -> usize {
  match level {
    Level::Trace => 0,
    Level::Debug => 1,
    Level::Info => 2,
    Level::Warn => 3,
    Level::Error => 4,
  }
}

/// The default log message format used.
pub fn default_format(now: SystemTime, record: &Record<'_>) -> Option<String> {
  use std::fmt::Write;
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
    /// Unambiguous date format with Month names
    const FORMAT: chrono::format::StrftimeItems<'_> =
      chrono::format::StrftimeItems::new("%d %b %Y %H:%M:%S%.3f UTC");
    chrono::DateTime::from_timestamp_millis(i64::try_from(instant).unwrap_or(0))
      .map(|dt| dt.format_with_items(FORMAT))?
  };

  if writeln!(
    buf,
    "{} - {} - {:?} - {}",
    prefix,
    instant,
    std::thread::current().id(),
    record.args()
  )
  .is_ok()
  {
    return Some(buf);
  }

  None
}

/// Returns the greatest `log::LevelFilter` possible that will still service all handlers fully.
pub fn get_level_for_handlers(handlers: &Vec<Box<dyn Handler>>) -> LevelFilter {
  let mut level = LevelFilter::Off;
  for handler in handlers {
    for lf in [Level::Error, Level::Warn, Level::Info, Level::Debug, Level::Trace] {
      if handler.is_enabled(lf) {
        level = std::cmp::max(lf.to_level_filter(), level);
      }
    }
  }
  level
}

#[cfg(test)]
mod test {
  use crate::util::get_level_for_handlers;
  use crate::Handler;

  use log::LevelFilter;

  #[test]
  fn level_for_handlers() {
    struct FakeLevelHandler {
      lf: LevelFilter,
    }
    impl FakeLevelHandler {
      fn new(lf: LevelFilter) -> Box<dyn Handler + 'static> {
        Box::new(Self { lf })
      }
    }
    impl Handler for FakeLevelHandler {
      fn log(&self, _now: std::time::SystemTime, _record: &log::Record<'_>) {}

      fn is_enabled(&self, level: log::Level) -> bool {
        level <= self.lf
      }
    }

    struct OnlyDebug;
    impl OnlyDebug {
      fn new() -> Box<dyn Handler + 'static> {
        Box::new(Self {})
      }
    }
    impl Handler for OnlyDebug {
      fn log(&self, _now: std::time::SystemTime, _record: &log::Record<'_>) {}

      fn is_enabled(&self, level: log::Level) -> bool {
        level == log::Level::Debug
      }
    }

    let handlers =
      vec![FakeLevelHandler::new(LevelFilter::Debug), FakeLevelHandler::new(LevelFilter::Info)];
    assert_eq!(get_level_for_handlers(&handlers), LevelFilter::Debug);

    let handlers = vec![
      FakeLevelHandler::new(LevelFilter::Off),
      FakeLevelHandler::new(LevelFilter::Info),
      FakeLevelHandler::new(LevelFilter::Trace),
      FakeLevelHandler::new(LevelFilter::Info),
    ];
    assert_eq!(get_level_for_handlers(&handlers), LevelFilter::Trace);

    let handlers = vec![
      FakeLevelHandler::new(LevelFilter::Off),
      FakeLevelHandler::new(LevelFilter::Error),
      FakeLevelHandler::new(LevelFilter::Warn),
    ];
    assert_eq!(get_level_for_handlers(&handlers), LevelFilter::Warn);

    let handlers = vec![
      OnlyDebug::new(),
      FakeLevelHandler::new(LevelFilter::Off),
      FakeLevelHandler::new(LevelFilter::Warn),
    ];
    assert_eq!(get_level_for_handlers(&handlers), LevelFilter::Debug);
  }
}
