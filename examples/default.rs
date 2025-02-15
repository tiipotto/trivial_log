use log::{debug, error, info, LevelFilter};

fn main() {
  trivial_log::init_std(LevelFilter::Trace).unwrap();

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
