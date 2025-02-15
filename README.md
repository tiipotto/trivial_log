# Trivial
This is intended to be a no-bloat implementation for [log](https://github.com/rust-lang/log). Unlike many other implementations, this crate intends to have no possible memory leaks (even 'static).

# Examples

## [default](./examples/default.rs#L4)
One line to get started with sane defaults for std logs (warn and below to stdout, errors to stderr).

```rust
trivial_log::init_std(LevelFilter::Trace).unwrap();
```

## [trivial](./examples/trivial.rs)
A more advanced configuration that includes both stdout and logging to a file

## [database](./examples/database.rs)
An "advanced" configuration, showing how to extend beyond the basics, by logging into a SQLite database.
