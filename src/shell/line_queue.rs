//! Minimal multi-producer line queue replacing `std::sync::mpsc` for [`Line`]
//! streaming between subprocess reader threads and the rendering loop.
//!
//! std's channel lazily switches between three internal implementations
//! (zero/array/list) and carries waker/context machinery — ~5.5 KiB of
//! monomorphized code per payload type. This use case is just "push lines,
//! pop with a frame-tick timeout", which a `Mutex<VecDeque>` + `Condvar`
//! covers in a fraction of the code.
//!
//! Semantics mirror the `mpsc` subset the shell module used: senders never
//! block or fail (lines queue unboundedly; a dropped receiver just means the
//! queue drains at join time), and the receiver reports disconnection only
//! once every sender is dropped *and* the queue is empty.

use std::{
   collections::VecDeque,
   sync::{Arc, Condvar, Mutex},
   time::{Duration, Instant}
};

use super::exec::Line;

struct State {
   items: VecDeque<Line>,
   senders: usize
}

struct Shared {
   state: Mutex<State>,
   ready: Condvar
}

/// Create a connected sender/receiver pair.
pub(super) fn line_channel() -> (LineSender, LineReceiver) {
   let shared =
      Arc::new(Shared { state: Mutex::new(State { items: VecDeque::new(), senders: 1 }), ready: Condvar::new() });
   (LineSender(Arc::clone(&shared)), LineReceiver(shared))
}

pub(super) struct LineSender(Arc<Shared>);

pub(super) struct LineReceiver(Arc<Shared>);

/// Why [`LineReceiver::recv_timeout`] returned without a line.
pub(super) enum RecvTimeout {
   /// No line arrived within the timeout; senders are still connected.
   Timeout,
   /// Every sender is dropped and the queue is empty — no more lines ever.
   Disconnected
}

impl LineSender {
   pub(super) fn send(&self, line: Line) {
      let mut state = self.0.state.lock().unwrap();
      state.items.push_back(line);
      drop(state);
      self.0.ready.notify_one();
   }
}

impl Clone for LineSender {
   fn clone(&self) -> Self {
      self.0.state.lock().unwrap().senders += 1;
      Self(Arc::clone(&self.0))
   }
}

impl Drop for LineSender {
   fn drop(&mut self) {
      let mut state = self.0.state.lock().unwrap();
      state.senders -= 1;
      let disconnected = state.senders == 0;
      drop(state);
      if disconnected {
         self.0.ready.notify_all();
      }
   }
}

impl LineReceiver {
   /// Block until a line arrives, every sender is dropped, or `timeout` elapses.
   pub(super) fn recv_timeout(&self, timeout: Duration) -> Result<Line, RecvTimeout> {
      let deadline = Instant::now() + timeout;
      let mut state = self.0.state.lock().unwrap();
      loop {
         if let Some(line) = state.items.pop_front() {
            return Ok(line);
         }
         if state.senders == 0 {
            return Err(RecvTimeout::Disconnected);
         }
         // Re-check the deadline on every pass so spurious wakeups can't
         // extend the wait.
         let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return Err(RecvTimeout::Timeout);
         };
         state = self.0.ready.wait_timeout(state, remaining).unwrap().0;
      }
   }

   /// Block until a line arrives; `None` once every sender is dropped and the
   /// queue is drained.
   fn recv(&self) -> Option<Line> {
      let mut state = self.0.state.lock().unwrap();
      loop {
         if let Some(line) = state.items.pop_front() {
            return Some(line);
         }
         if state.senders == 0 {
            return None;
         }
         state = self.0.ready.wait(state).unwrap();
      }
   }
}

impl Iterator for LineReceiver {
   type Item = Line;

   fn next(&mut self) -> Option<Line> {
      self.recv()
   }
}

#[cfg(test)]
mod tests {
   use std::thread;

   use super::*;

   fn text(line: Line) -> String {
      match line {
         Line::Stdout(s) | Line::StdoutCr(s) | Line::Stderr(s) => s
      }
   }

   #[test]
   fn lines_arrive_in_send_order() {
      let (tx, rx) = line_channel();
      tx.send(Line::Stdout("a".into()));
      tx.send(Line::Stderr("b".into()));
      drop(tx);

      let texts: Vec<String> = rx.map(text).collect();

      assert_eq!(texts, vec!["a", "b"]);
   }

   #[test]
   fn iteration_ends_when_all_senders_drop() {
      let (tx, rx) = line_channel();
      let tx2 = tx.clone();
      drop(tx);
      tx2.send(Line::Stdout("late".into()));
      drop(tx2);

      let texts: Vec<String> = rx.map(text).collect();

      assert_eq!(texts, vec!["late"]);
   }

   #[test]
   fn recv_timeout_times_out_while_senders_live() {
      let (tx, rx) = line_channel();

      let result = rx.recv_timeout(Duration::from_millis(10));

      assert!(matches!(result, Err(RecvTimeout::Timeout)));
      drop(tx);
      assert!(matches!(rx.recv_timeout(Duration::from_millis(10)), Err(RecvTimeout::Disconnected)));
   }

   #[test]
   fn recv_timeout_wakes_on_cross_thread_send() {
      let (tx, rx) = line_channel();
      let sender = thread::spawn(move || {
         thread::sleep(Duration::from_millis(20));
         tx.send(Line::Stdout("woke".into()));
      });

      let line = rx.recv_timeout(Duration::from_secs(5)).ok().map(text);

      sender.join().unwrap();
      assert_eq!(line.as_deref(), Some("woke"));
   }

   #[test]
   fn drained_queue_still_yields_buffered_lines_after_disconnect() {
      let (tx, rx) = line_channel();
      tx.send(Line::Stdout("buffered".into()));
      drop(tx);

      let line = rx.recv_timeout(Duration::from_millis(10));

      assert!(matches!(line, Ok(Line::Stdout(s)) if s == "buffered"));
   }
}
