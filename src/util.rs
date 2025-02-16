use std::time::{SystemTime, UNIX_EPOCH};

use crate::{Handler, Level, LevelFilter};
use log::Record;

pub(crate) fn default_format(now: SystemTime, record: &Record<'_>) -> Option<String> {
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
    const FORMAT: chrono::format::StrftimeItems<'_> =
      chrono::format::StrftimeItems::new("%d.%m.%Y %H:%M:%S%.3f UTC");
    chrono::DateTime::from_timestamp_millis(i64::try_from(instant).unwrap_or(0))
      .map(|dt| dt.format_with_items(FORMAT))?
  };

  use std::fmt::Write;
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

pub(crate) fn get_level_for_handlers(handlers: &Vec<Box<dyn Handler>>) -> LevelFilter {
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

#[cfg(test)]
mod test {}
