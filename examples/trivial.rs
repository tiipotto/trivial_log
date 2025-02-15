use log::{debug, error, info, Level, LevelFilter, Record};
use std::fs::{File, OpenOptions};
use std::io::{stdout, Write};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

fn printer(buf: &mut Box<dyn Write + Send>, record: &Record<'_>) {
    let prefix = match record.metadata().level() {
        Level::Error => "[E]",
        Level::Info => "[I]",
        _ => "[Cat]",
    };

    writeln!(
        buf,
        "{} {:?} {}",
        prefix,
        std::thread::current().id(),
        record.args().to_string()
    )
    .unwrap()
}

fn main() {
    _= trivial_log::builder()
        .appender(Level::Error, OpenOptions::new().append(true).create(true).open("/tmp/shitlog.log").unwrap())
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
