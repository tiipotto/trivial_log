use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::{mpsc, Arc, Mutex};

use crate::{Appender, IntoAppender};

/// Appender for `BufWriter`
struct AppenderWriter<X: Write + Send>(Mutex<BufWriter<X>>);

impl<T, Y> IntoAppender<Y> for T
where
  T: Appender<Y> + 'static,
{
  fn into_appender(self) -> Arc<dyn Appender<Y>> {
    Arc::new(self)
  }
}

impl<T, Y> Appender<Y> for T
where
  T: Fn(&Y) + Send + Sync,
{
  fn append_log_message(&self, message: &Y) {
    self(message);
  }
}

impl<X: Write + Send> Appender<String> for AppenderWriter<X> {
  fn append_log_message(&self, message: &String) {
    if let Ok(mut guard) = self.0.lock() {
      // We ignore errors
      _ = guard.write_all(message.as_bytes());
      _ = guard.flush();
    }
  }
}

impl<X: Write + Send + 'static> IntoAppender<String> for BufWriter<X> {
  fn into_appender(self) -> Arc<dyn Appender<String>> {
    Arc::new(AppenderWriter(Mutex::new(self)))
  }
}

impl IntoAppender<String> for File {
  fn into_appender(self) -> Arc<dyn Appender<String>> {
    Arc::new(AppenderWriter(Mutex::new(BufWriter::new(self))))
  }
}

impl IntoAppender<String> for &Path {
  fn into_appender(self) -> Arc<dyn Appender<String>> {
    match OpenOptions::new().append(true).create(true).open(self) {
      Ok(file) => file.into_appender(),
      Err(err) => {
        panic!("Failed to open or create log file {} reason: {}", self.to_string_lossy(), err)
      }
    }
  }
}

impl<X: Send + Clone> Appender<X> for mpsc::Sender<X> {
  fn append_log_message(&self, message: &X) {
    // We don't care if the receiving end is dead.
    _ = self.send(message.clone());
  }
}

impl<X: Send + Clone> Appender<X> for mpsc::SyncSender<X> {
  fn append_log_message(&self, message: &X) {
    // We don't care if the receiving end is dead.
    _ = self.send(message.clone());
  }
}
