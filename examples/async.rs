use log::{debug, error, info, LevelFilter};
use std::sync::mpsc;
use std::thread;

fn main() {
  let (snd, rcv) = mpsc::channel::<String>();
  trivial_log::builder()
    .default_format(|builder| builder.appender_filter(LevelFilter::Trace, snd))
    .init()
    .unwrap();

  let jh2 = thread::spawn(move || loop {
    let Ok(msg) = rcv.recv() else {
      return;
    };
    print!("{msg}");
  });

  error!("An error has occurred, please help!");
  println!("normal println");
  debug!("warning");
  let t = thread::spawn(move || {
    info!("猫");
  });
  t.join().unwrap();

  trivial_log::free();

  jh2.join().unwrap();
}

#[test]
fn run() {
  main();
}
