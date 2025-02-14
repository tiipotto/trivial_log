use log::{debug, error, info, Level};
use std::path::Path;

fn main() {
  trivial_log::builder()
    .default_format(|builder| {
      builder
        //
        .appender_range(Level::Info, Level::Error, Path::new("mylog.log"))
        .appender_range(Level::Trace, Level::Error, |msg: &String| print!("{msg}"))
    })
    .init()
    .unwrap();

  error!("An error has occurred, please help!");
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
