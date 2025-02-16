# Trivial
This is intended to be a no-bloat implementation for [log](https://github.com/rust-lang/log).
It includes simple defaults while still providing good flexibility for more advanced use cases.

# Motivation
The original motivation was logging while running valgrind tests that treat "possibly leaked" as errors.
Unlike many other implementations, this crate intends to have no possible memory leaks (even 'static).
All examples should have `All heap blocks were freed -- no leaks are possible`.

# Examples

## [stdout/stderr](./examples/default.rs)
One line to get started with sane defaults for std logs (warn and below to stdout, errors to stderr).

```rust
fn main() {
    trivial_log::init_std(LevelFilter::Trace).unwrap();
    error!("An error has occurred, please help!");
}
```

This will cause messages like these to appear:
```text
[E] - 17 Feb 2025 22:27:20.956 UTC - ThreadId(1) - An error has occurred, please help!
```

## [File](./examples/file.rs)
A more advanced configuration that includes both stdout and logging to a file
* Info to Error is logged to "mylog.log"
* Trace to Error is logged to stdout.

```rust
fn main() {
    trivial_log::builder()
        .default_format(|builder| {
            builder
                //
                .appender_range(Level::Info, Level::Error, Path::new("mylog.log"))
                .appender_range(Level::Trace, Level::Error, |msg: &String| print!("{}", msg))
        })
        .init()
        .unwrap();

    error!("An error has occurred, please help!");
}

```

## [Stdout+JSON](./examples/json.rs)
An "advanced" configuration that shows how to log normal human-readable messages to stdout
but shows how to prepare a json object for "processing" (eg to a log server).

```rust
/// This serves as an example of how your json entity may look like.
/// You may design this struct in any ways you see fit.
#[derive(Serialize, Deserialize)]
struct LogEntity {
    message: String,
    level: String,
    thread: String,
    timestamp: u128,
}

fn main() {
    trivial_log::builder()
        .default_format(|builder| builder.appender_range(Level::Trace, Level::Error, |msg: &String| print!("{}", msg)))
        .format(|now: std::time::SystemTime, record: &log::Record| {
            Some(LogEntity {
                message: record.args().to_string(),
                level: record.level().to_string(),
                thread: format!("{:?}", std::thread::current().id()),
                timestamp: now.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis(),
            })
        }, |builder| builder.appender_range(Level::Trace, Level::Error, |msg: &LogEntity| print!("{}", serde_json::to_string(msg).unwrap())))
        .init()
        .unwrap();
}

```

## [Database](./examples/database.rs)
An "advanced" configuration, showing how to implement a custom Appender Implementation, by logging into a SQLite database.

## [Colors](./examples/color.rs)
On ANSI terminals you can write colors.
This examples uses the ansi_term crate, but you can also create the ansi escape codes manually without any dependencies.

```rust
fn main() {
    use ansi_term::Color;
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
            |builder| builder.appender_filter(LevelFilter::Trace, |msg: &String| print!("{}", msg)),
        )
        .init()
        .ok();
}
```

# Architecture
The logging is split into 2 parts.
1. The format function, which processes the log::Record along with a timestamp into an arbitrary struct of your choosing.
The default format outputs String.
2. The appender, which writes the output of the format function to somewhere (stdout/file/...) if the level of the record
matches the level of the appender.

There is no limit for how many format functions or appenders you can use.

Each log always processes all formats and appenders when the log level matches.
When a format has multiple appenders, the format fn only gets called once.

# Default Appender Implementations
* `std::io::BufWriter<T> where T: Write + Send` - io errors are ignored
* `std::path::Path` - inability to open or create the file will panic! Other io errors are ignored.
* `std::fs::File` - io errors are ignored
* `std::sync::mpsc::Sender<T> where T: Send+Clone` - if the receiver dies then this appender becomes a noop.
* `std::sync::mpsc::SyncSender<T> where T: Send+Clone` - if the receiver dies then this appender becomes a noop.
  The appender only uses the send method to send data

# Implementation details
1. The appender's and formats can be reconfigured at any time during the application.
2. trivial_log does NOT prevent recursive calls inside the appender.
   * It's the responsibility of the appender to prevent calls to `log!` from inside the appender that can lead to a stack overflow.
3. trivial_log does NOT catch panics that occur in the appender.
   * Panics are propangated to caller of the `log!` function.
     Either use panic=abort, or prevent/catch panics in the appender impl as the caller of `log!` is unlikely to expect it to panic.
4. trivial_log does NOT start any threads.
   * If an appender can take a very long time (e.g. logging over a network), it may be a good idea to use a background thread.
     See the [async example](./examples/async.rs) for a starting point.
5. trivial_log does NOT do any synchronization
  * The appender impl has to synchronize to prevent concurrent access to mutable resources (such as a file/stream).
    This crate uses no unsafe, so the rust compiler will prevent misuse.
    See [Database](./examples/database.rs) for an example.
  * Your appender will be called concurrently if multiple concurrent threads call log at the same.
    It is up to the appender implementation on what to do in this case.
    * The default impl for file will acquire an ordinary Mutex in the appender
    * The default impl for stdout/stderr will call print! and eprint! macros which guarantee synchronization.
6. For no "possibly leaked" in valgrind, you must call `trivial_log::free()` before the process exits. It is memory safe (no UB) to not call this.
   * as documented in the trivial_log::free() fns documentation, calling this fn will not cause any problems when after it you "accidentally" still call log!. You just won't see those log messages.
7. The default format (which you can easily customize) will always output UTC time. 
   * I understand this may be inconvenient to people that work with only one time zone but for people that have to compare logs from servers in several different time zones this is a godsend! 