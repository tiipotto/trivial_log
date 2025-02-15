use log::{debug, error, info, Level};
use std::fs::OpenOptions;

fn main() {
  trivial_log::builder()
    .appender(
      Level::Error,
      OpenOptions::new().append(true).create(true).open("shitlog.log").unwrap(),
    )
    .appender_range(Level::Trace, Level::Error, |msg: &str| println!("{}", msg))
    .init()
    .unwrap();

  error!("fuck you");
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
