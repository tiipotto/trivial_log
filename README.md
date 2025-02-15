# Trivial
This is intended to be a no-bloat implementation for [log](https://github.com/rust-lang/log). Unlike many other implementations, this crate intends to have no possible memory leaks (even 'static).

# Examples

## [default](./examples/default.rs#L4-L6)
    Just three lines to get reasonable logs out stdout.

## [trivial](./examples/trivial.rs)
    A more advanced configuration that includes both stdout and logging to a file

## [database](./examples/database.rs)
    An "advanced" configuration, showing how to extend beyond the basics, by logging into a SQLite database.
