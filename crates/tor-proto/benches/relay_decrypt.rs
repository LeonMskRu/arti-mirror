use criterion::{criterion_group, criterion_main, measurement::Measurement, Criterion, Throughput};
use criterion_cycles_per_byte::CyclesPerByte;
use rand::prelude::*;

#[cfg(feature = "counter-galois-onion")]
use aes::{Aes128Dec, Aes128Enc, Aes256Dec, Aes256Enc};
use tor_bytes::SecretBuf;
use tor_llcrypto::{
    cipher::aes::{Aes128Ctr, Aes256Ctr},
    d::{Sha1, Sha3_256},
};
#[cfg(feature = "counter-galois-onion")]
use tor_proto::bench_utils::cgo;
use tor_proto::bench_utils::{tor1, OutboundClientCryptWrapper, RelayBody, RelayCryptState};

const HOP_NUM: u8 = 1;

/// Helper macro to set up a relay decryption benchmark.
macro_rules! relay_decrypt_setup {
    ($client_state_construct: path, $relay_state_construct: path) => {{
        let seed1: SecretBuf = b"hidden we are free".to_vec().into();
        let seed2: SecretBuf = b"free to speak, to free ourselves".to_vec().into();

        // No need to simulate other relays since we are only one relay.
        let relay_state = $relay_state_construct(seed1.clone()).unwrap();

        let mut cc_out = OutboundClientCryptWrapper::new();
        let state1 = $client_state_construct(seed1).unwrap();
        cc_out.add_layer(state1);
        // Add a second layer to avoid the benched relay to recognize the relay cell.
        let state2 = $client_state_construct(seed2).unwrap();
        cc_out.add_layer(state2);

        let mut rng = rand::rng();
        let mut cell = [0u8; 509];
        rng.fill(&mut cell[..]);
        let mut cell: RelayBody = cell.into();
        cc_out.encrypt(&mut cell, HOP_NUM).unwrap();
        (cell, relay_state)
    }};
}

/// Benchmark a relay decrypting a relay cell coming from the client.
pub fn relay_decrypt_benchmark(c: &mut Criterion<impl Measurement>) {
    // Group for the Tor1 relay crypto with 498 bytes of data per relay cell.
    let mut group = c.benchmark_group("relay_decrypt");
    group.throughput(Throughput::Bytes(498));

    group.bench_function("Tor1RelayCrypto", |b| {
        b.iter_batched_ref(
            || {
                relay_decrypt_setup!(
                    tor1::Tor1ClientCryptState::<Aes128Ctr, Sha1>::construct,
                    tor1::Tor1RelayCryptState::<Aes128Ctr, Sha1>::construct
                )
            },
            |(cell, relay_state)| {
                relay_state.decrypt(cell);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("Tor1Hsv3RelayCrypto", |b| {
        b.iter_batched_ref(
            || {
                relay_decrypt_setup!(
                    tor1::Tor1ClientCryptState::<Aes256Ctr, Sha3_256>::construct,
                    tor1::Tor1RelayCryptState::<Aes256Ctr, Sha3_256>::construct
                )
            },
            |(cell, relay_state)| {
                relay_state.decrypt(cell);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();

    // Group for the Counter-Galois-Onion relay crypto with ~488 bytes of data per relay cell.
    let mut group = c.benchmark_group("relay_decrypt");
    group.throughput(Throughput::Bytes(488));

    #[cfg(feature = "counter-galois-onion")]
    group.bench_function("CGO_Aes128", |b| {
        b.iter_batched_ref(
            || {
                relay_decrypt_setup!(
                    cgo::CgoClientCryptState::<Aes128Dec, Aes128Enc>::construct,
                    cgo::CgoRelayCryptState::<Aes128Enc, Aes128Enc>::construct
                )
            },
            |(cell, relay_state)| {
                relay_state.decrypt(cell);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    #[cfg(feature = "counter-galois-onion")]
    group.bench_function("CGO_Aes256", |b| {
        b.iter_batched_ref(
            || {
                relay_decrypt_setup!(
                    cgo::CgoClientCryptState::<Aes256Dec, Aes256Enc>::construct,
                    cgo::CgoRelayCryptState::<Aes256Enc, Aes256Enc>::construct
                )
            },
            |(cell, relay_state)| {
                relay_state.decrypt(cell);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(
    name = relay_decrypt;
    config = Criterion::default()
       .with_measurement(CyclesPerByte)
       .sample_size(5000);
    targets = relay_decrypt_benchmark);
criterion_main!(relay_decrypt);
