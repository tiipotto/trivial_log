use log::{debug, error, info, Level};

fn main() {
  _ = trivial_log::builder()
    .appender_range(Level::Trace, Level::Error, |msg: &str| println!("{}", msg))
    .init();

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
