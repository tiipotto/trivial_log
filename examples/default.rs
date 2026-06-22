use log::{debug, error, info, LevelFilter};

fn main() {
  trivial_log::init_std(LevelFilter::Trace).unwrap();

  error!("a critical error");
  println!("normal println");
  debug!("warning");
  let t = std::thread::spawn(move || {
    info!("猫 playing in your threads");
  });
  t.join().unwrap();

  trivial_log::free();
}

#[test]
fn run() {
  main();
}
