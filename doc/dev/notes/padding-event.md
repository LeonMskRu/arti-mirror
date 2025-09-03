## Circuit reactor padding event handling

## Conflux considerations

Tunnel reactors (`tor_proto::client::reactor`) handle 1 or more circuits.
Because the padding logic is per-circuit, we need the `PaddingEventStream` to be
part of `Circuit`[^1], and not of the tunnel reactor (so we'll need to change the
`Circuit` constructor to take the `PaddingEventStream` as an argument).

[^1]: `tor_proto::client::circuit::Circuit` is the internal representation of a
client circuit in the tunnel reactor

## Handling events

> This whole document assumes that we will be implementing
> a simplified version of the padding algorithm,
> where `StartBlocking` events apply to the whole circuit,
> and not on a per-hop basis.

The main driver of the tunnel reactor is `ConfluxSet::next_circ_action()`.
This function polls all the relevant (rust) channels of all the circuits
in the conflux set, and returns a `CircuitAction` that gets handled
in the tunnel reactor main loop. This function is the place where we need
to poll the `PaddingEvenStream` for events (more specifically,
we need to read padding events unconditionally, so we will need to
call `padding_events.next()` in the `select_biased!` that
polls `conflux_hs_timeout` and `send_fut`).

The way these events should be handled is described below.

### `SendPadding`

A `SendPadding` event means we will need to send a padding
cell after a given timer elapses.

> TODO(gabi): I am not sure I understand how `SendPadding::limit`
> is supposed to be used.

So to handle a `SendPadding` event, we will need to install
a timer inside `Circuit`. This means `Circuit` will need to have
a collection of such timers, and an API that `ConfluxSet` can use
to poll all of these timers. I imagine this API will look roughly
like this

```rust
    pub async fn next_padding(&mut self) -> PaddingInstructions {
        // Call futures::future::select_all(),
        // passing our set of timer futures as an argument.
        //
        // select_all() returns the index of the resolved future,
        // which will be used to remove said timer from our set
        // after it elapses.
    }
```

where `PaddingInstructions` is a struct that contains the information
from `SendPadding` (`bypass`, `replace`?) needed to build the padding
command to send to the reactor.

Alternatively, we can have an API that is similar to that of
`Circuit::conflux_hs_timeout()`, which returns the `SystemTime`
(or ideally `Instant`) until which to sleep (the API will return
need to return the soonest timeout, so we could use a min-heap
or something here).

> I suspect we will need to go with the second option,
> and (re)create the timer futures inside the `select_biased!`
> from `next_circ_action()`, because the first option
> would, AFAICT, involve borrowing all the `Circuit`s from the
> `ConfluxSet` mutably at the same time, which Rust
> won't allow us to do.

TODO: figure out a more elegant approach here?

In any case, `leg.next_padding()` will need to be called
unconditionally inside `next_circ_action()`, alongside the
call to `padding_events.next()`.

Here is a sketch of what the `select_biased!` from `next_circ_action()` might
end up looking like:

```rust

                async move {
                    select_biased! {
                        () = conflux_hs_timeout.fuse() => { .. }
                        event = leg.padding_events.next() => {
                            match event {
                                SendPadding => {
                                    // install timer in circuit
                                },
                                StartBlocking => { /* TODO */ }
                                StopBlocking => { /* TODO */ }
                            }
                        }
                        padding_instructions = leg.next_padding() => {
                            // Build a CircuitCmd::SendPadding circuit action
                            // that will be handled by the circuit reactor
                            let cmd = CircuitCmd::SendPadding {
                                cell,
                                bypass: padding_instructions.bypass,
                                replace: padding_instructions.replace
                            };

                            // Tell the reactor to run this CircuitCmd
                            Ok(Ok(CircuitAction::RunCmd { leg: unique_id, cmd }))
                        }
                        ret = send_fut => { .. }
                    }
```

`PaddingInstructions` need to be mapped to a new a new kind of `CircuitCmd`
called `CircuitCmd::SendPadding` (or similar). The `SendPadding` variant
will look roughly like this

```rust
    /// Send a padding cell.
    SendPadding {
        /// The leg the cell should be sent on.
        leg: UniqId,
        /// The cell to send.
        cell: SendRelayCell,
        /// Whether to bypass a bypassable block.
        bypass: bool,
        /// Whether this can be replaced by a non-padding cell
        replace: bool,
    },
```

> Important! We must ensure `cmd_counts_towards_seqno(cmd)` returns `false`
> padding `cmd`s. Otherwise, on multi-path tunnels, the cell won't get sent
> on the right leg, and will instead get rerouted by the conflux logic
> to the primary (sending) leg of the tunnel.
>
> I believe this requirement is already satisfied though
> (in `cmd_counts_towards_seqno`, `RelayCmd::Drop` is mapped to `false`)


### `StartBlocking` and `StopBlocking`

TODO: this will be implemented as a flag in the circuit's
Sender/Receivers. I think @nickm has some ideas here.
