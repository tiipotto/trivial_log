use ansi_term::Color;
use log::{debug, error, info, trace, warn, Level, LevelFilter};

fn main() {
  trivial_log::builder()
    .format(
      |_time, rec| {
        let prefix = match rec.level() {
          Level::Error => Color::Red.paint("E"),
          Level::Warn => Color::Yellow.paint("W"),
          Level::Info => Color::Green.paint("I"),
          Level::Debug => Color::Purple.paint("D"),
          Level::Trace => Color::White.paint("T"),
        };
        Some(format!(
          "{}{}{} {}\n",
          Color::Blue.paint("["),
          prefix,
          Color::Blue.paint("]"),
          rec.args()
        ))
      },
      |builder| builder.appender_filter(LevelFilter::Trace, |msg: &String| print!("{msg}")),
    )
    .init()
    .ok();

  error!("Error");
  warn!("Warn");
  info!("Info");
  debug!("Debug");
  trace!("Trace");

  trivial_log::free();
}
