//! [`SometimesUnboundedSink`]

use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{
    Context,
    Poll::{self, *},
    Waker, ready,
};

use futures::{Sink, future};

use pin_project::pin_project;

/// Wraps a [`Sink`], providing an only-sometimes-used unbounded buffer
///
/// For example, consider `SometimesUnboundedSink<T, mpsc::Receiver>`.
/// The `Receiver` is not always ready for writing:
/// if the capacity is exceeded, `send` will block.
///
/// `SometimesUnboundedSink`'s `Sink` implementation works the same way.
/// But there are also two methods
/// [`pollish_send_unbounded`](SometimesUnboundedSink::pollish_send_unbounded)
/// and
/// [`send_unbounded`](SometimesUnboundedSink::send_unbounded)
/// which will always succeed immediately.
/// Items which the underlying sink `S` is not ready to accept are queued,
/// and will be delivered to `S` when possible.
///
/// ### You must poll this type
///
/// For queued items to be delivered,
/// `SometimesUnboundedSink` must be polled,
/// even if you don't have an item to send.
///
/// You can use [`Sink::poll_ready`] for this.
/// Any [`Context`]-taking methods is suitable.
///
/// ### Blocking
///
/// A `SometimesUnboundedSink` may be _blocked_.
/// While it is blocked, it always returns Pending for bounded-send requests,
/// even if the underlying sink can receive.
/// (Unbounded-send requests are still allowed.)
///
/// Additionally, while the sink is blocked,
/// only a limited number of cells will be flushed
/// from the `SometimesUnboundSink`'s internal queue onto the underlying sink.
/// This number can be adjusted with [`allow_flush`](Self::allow_flush).
///
/// ### Error handling
///
/// Errors from the underlying sink may not be reported immediately,
/// due to the buffering in `SometimesUnboundedSink`.
///
/// However, if the sink reports errors from `poll_ready`
/// these will surface in a timely fashion.
///
/// After an error has been reported, there may still be buffered data,
/// which will only be delivered if `SometimesUnboundedSink` is polled again
/// (and the error in the underlying sink was transient).
//
// TODO circpad: Depending on what we need to add in order to implement circuit padding,
// we might need to allow `buf` to hold a certain capacity even in response
// to regular bounded send.  (In other words, when the sink is full,
// we'd let people queue up to N items on our buf with a regular poll_ready.)
//
// But we won't build that if we don't have to. This logic will need to be changed anyway
// when we finally implement circuit muxes.
#[pin_project]
pub(crate) struct SometimesUnboundedSink<T, S> {
    /// Things we couldn't send_unbounded right away
    ///
    /// Invariants:
    ///
    ///  * Everything here must be fed to `inner` before any further user data
    ///    (unbounded user data may be appended).
    ///
    ///  * If this is nonempty, the executor knows to wake this task.
    ///    This is achieved as follows:
    ///    If this is nonempty, `inner.poll_ready()` has been called.
    ///
    ///    XXXX no longer true; what to say instead?
    buf: VecDeque<T>,

    /// If true, we should behave as if the underlying sink is blocked,
    /// _whether it is truly blocked or not_.
    ///
    /// That means that our own poll_ready() always returns Pending.
    /// Additionally, our own attempts to flush onto the sink always
    /// return Pending unless n_flush_bypass can be decremented.
    ///
    /// Invariants:
    ///  * If this has been cleared, the executor knows to wake this task.
    ///    We guarantee this by invoking `waker` whenever we clear it.
    ///  * XXXX: what else?
    blocked: bool,

    /// A number of cells that we can flush in spite of `blocked`.
    ///
    /// Invariants:
    ///  * This is 0 whenever blocked is false.
    n_flush_bypass: usize,

    /// A waker that we alert whenever our blocking status has become
    /// more permissive.
    ///
    /// Invariants:
    ///  * This is None whenever blocked is false.
    ///  * This can only transition from Some to None by waking it.
    waker: Option<Waker>,

    /// The actual sink
    ///
    /// This also has the relevant `Waker`.
    ///
    /// # Waker invariant
    ///
    /// Whenever either
    ///
    ///  * The last call to any of our public methods returned `Pending`, or
    ///  * `buf` is nonempty,
    ///
    /// the last method call `inner` *also* returned `Pending`.
    /// (Or, we have reported an error.)
    ///
    /// So, in those situations, this task has been recorded for wakeup
    /// by `inner` (specifically, its other end, if it's a channel)
    /// when `inner` becomes readable.
    ///
    /// Therefore this task will be woken up, and, if the caller actually
    /// polls us again (as is usual and is required by our docs),
    /// we'll drain any queued data.
    #[pin]
    inner: S,
}

impl<T, S: Sink<T>> SometimesUnboundedSink<T, S> {
    /// Wrap an inner `Sink` with a `SometimesUnboundedSink`
    //
    // There is no method for unwrapping.  If we make this type more public,
    // there should be, but that method will need `where S: Unpin`.
    pub(crate) fn new(inner: S) -> Self {
        SometimesUnboundedSink {
            buf: VecDeque::new(),
            blocked: false,
            n_flush_bypass: 0,
            waker: None,
            inner,
        }
    }

    /// Return the number of T queued in this sink.
    pub(crate) fn n_queued(&self) -> usize {
        self.buf.len()
    }

    /// Hand `item` to the inner Sink if possible, or queue it otherwise
    ///
    /// Like a `poll_...` method in that it takes a `Context`.
    /// That's needed to make sure we get polled again
    /// when the underlying sink can accept items.
    ///
    /// But unlike a `poll_...` method in that it doesn't return `Poll`,
    /// since completion is always immediate.
    pub(crate) fn pollish_send_unbounded(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        item: T,
    ) -> Result<(), S::Error> {
        match self.as_mut().poll_ready(cx) {
            // Waker invariant: poll_ready only returns Ready(Ok(())) if `buf` is empty
            Ready(Ok(())) => self.as_mut().start_send(item),
            // Waker invariant: if we report an error, we're then allowed to expect polling again
            Ready(Err(e)) => Err(e),
            Pending => {
                // Waker invariant: poll_ready() returned Pending,
                // so the task has indeed already been recorded.
                self.as_mut().project().buf.push_back(item);
                Ok(())
            }
        }
    }

    /// Hand `item` to the inner Sink if possible, or queue it otherwise (async fn)
    ///
    /// You must `.await` this, but it will never block.
    /// (Its future is always `Ready`.)
    pub(crate) async fn send_unbounded(mut self: Pin<&mut Self>, item: T) -> Result<(), S::Error> {
        // Waker invariant: this is just a wrapper around `pollish_send_unbounded`
        let mut item = Some(item);
        future::poll_fn(move |cx| {
            let item = item.take().expect("polled after Ready");
            Ready(self.as_mut().pollish_send_unbounded(cx, item))
        })
        .await
    }

    /// Flush the buffer.  On a `Ready(())` return, it's empty.
    ///
    /// This satisfies the Waker invariant as if it were a public method.
    fn flush_buf(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), S::Error>> {
        let mut self_ = self.as_mut().project();
        while !self_.buf.is_empty() {
            if *self_.blocked && *self_.n_flush_bypass == 0 {
                // We don't want to flush, so we have to remember the waker.
                *self_.waker = Some(cx.waker().clone());
                return Pending;
            }
            // Waker invariant:
            // if inner gave Pending, we give Pending too: ok
            // if inner gave Err, we're allowed to want polling again
            ready!(self_.inner.as_mut().poll_ready(cx))?;
            let item = self_.buf.pop_front().expect("suddenly empty!");
            // Waker invariant: returning Err
            self_.inner.as_mut().start_send(item)?;
            *self_.n_flush_bypass = self_.n_flush_bypass.saturating_sub(1);
        }
        // Waker invariant: buffer is empty, and we're not about to return Pending
        Ready(Ok(()))
    }

    /// Mark this sink as blocked.
    ///
    /// While a sink is blocked,
    /// it acts as if it were full in response to all non-unbounded send requests,
    /// and it does not flush its internal queue
    /// except as described with [`allow_flush`](Self::allow_flush).
    #[allow(unused)] //XXXX
    pub(crate) fn set_blocked(&mut self) {
        self.blocked = true;
    }

    /// If this sink is blocked, allow `n` items to be flushed from the queue.
    #[allow(unused)] //XXXX
    pub(crate) fn allow_flush(&mut self, n: usize) {
        if self.blocked {
            self.n_flush_bypass = self.n_flush_bypass.saturating_add(n);
            if let Some(waker) = self.waker.take() {
                waker.wake();
            }
        }
    }

    /// Mark this sink as unblocked.
    #[allow(unused)] //XXXX
    pub(crate) fn set_unblocked(&mut self) {
        self.blocked = false;
        self.n_flush_bypass = 0;
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }

    /// Obtain a reference to the inner `Sink`, `S`
    ///
    /// This method should be used with a little care, since it bypasses the wrapper.
    /// For example, if `S` has interior mutability, and this method is used to
    /// modify it, the `SometimesUnboundedSink` may malfunction.
    pub(crate) fn as_inner(&self) -> &S {
        &self.inner
    }
}

// Waker invariant for all these impls:
// returning Err or Pending from flush_buf: OK, flush_buf ensures the condition holds
// returning from the inner method: trivially OK
impl<T, S: Sink<T>> Sink<T> for SometimesUnboundedSink<T, S> {
    type Error = S::Error;

    // Only returns `Ready(Ok(()))` if `buf` is empty
    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), S::Error>> {
        ready!(self.as_mut().flush_buf(cx))?;
        self.project().inner.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: T) -> Result<(), S::Error> {
        assert!(self.buf.is_empty(), "start_send without poll_ready");
        self.project().inner.start_send(item)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), S::Error>> {
        ready!(self.as_mut().flush_buf(cx))?;
        self.project().inner.poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), S::Error>> {
        ready!(self.as_mut().flush_buf(cx))?;
        self.project().inner.poll_close(cx)
    }
}

#[cfg(test)]
mod test {
    // @@ begin test lint list maintained by maint/add_warning @@
    #![allow(clippy::bool_assert_comparison)]
    #![allow(clippy::clone_on_copy)]
    #![allow(clippy::dbg_macro)]
    #![allow(clippy::mixed_attributes_style)]
    #![allow(clippy::print_stderr)]
    #![allow(clippy::print_stdout)]
    #![allow(clippy::single_char_pattern)]
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::unchecked_duration_subtraction)]
    #![allow(clippy::useless_vec)]
    #![allow(clippy::needless_pass_by_value)]
    //! <!-- @@ end test lint list maintained by maint/add_warning @@ -->
    use super::*;
    use futures::channel::mpsc;
    use futures::{SinkExt as _, StreamExt as _};
    use std::pin::pin;
    use tor_rtmock::MockRuntime;

    #[test]
    fn cases() {
        // `test_with_various` runs with both LIFO and FIFO scheduling policies,
        // so should interleave the sending and receiving tasks
        // in ways that exercise the corner cases we're interested in.
        MockRuntime::test_with_various(|runtime| async move {
            let (tx, rx) = mpsc::channel(1);
            let tx = SometimesUnboundedSink::new(tx);

            runtime.spawn_identified("sender", async move {
                let mut tx = pin!(tx);
                let mut n = 0..;
                let mut n = move || n.next().unwrap();

                // unbounded when we can send right away
                tx.as_mut().send_unbounded(n()).await.unwrap();
                tx.as_mut().send(n()).await.unwrap();
                tx.as_mut().send(n()).await.unwrap();
                tx.as_mut().send(n()).await.unwrap();
                // unbounded when we maybe can't and might queue
                tx.as_mut().send_unbounded(n()).await.unwrap();
                tx.as_mut().send_unbounded(n()).await.unwrap();
                tx.as_mut().send_unbounded(n()).await.unwrap();
                // some interleaving
                tx.as_mut().send(n()).await.unwrap();
                tx.as_mut().send_unbounded(n()).await.unwrap();
                // flush
                tx.as_mut().flush().await.unwrap();
                // close
                tx.as_mut().close().await.unwrap();
            });

            runtime.spawn_identified("receiver", async move {
                let mut rx = pin!(rx);
                let mut exp = 0..;

                while let Some(n) = rx.next().await {
                    assert_eq!(n, exp.next().unwrap());
                }
                assert_eq!(exp.next().unwrap(), 9);
            });

            runtime.progress_until_stalled().await;
        });
    }
}
