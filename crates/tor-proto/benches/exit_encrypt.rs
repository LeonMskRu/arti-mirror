use criterion::{criterion_group, criterion_main, measurement::Measurement, Criterion, Throughput};
use criterion_cycles_per_byte::CyclesPerByte;
use rand::prelude::*;

#[cfg(feature = "counter-galois-onion")]
use aes::{Aes128Enc, Aes256Enc};
use tor_bytes::SecretBuf;
use tor_llcrypto::{
    cipher::aes::{Aes128Ctr, Aes256Ctr},
    d::{Sha1, Sha3_256},
};
#[cfg(feature = "counter-galois-onion")]
use tor_proto::bench_utils::cgo;
use tor_proto::bench_utils::{tor1, RelayBody, RelayCryptState};

/// Helper macro to set up an exit encryption benchmark.
macro_rules! exit_encrypt_setup {
    ($relay_state_construct: path) => {{
        let seed1: SecretBuf = b"hidden we are free".to_vec().into();

        // No need to simulate other relays since we are only benchmarking the exit relay.
        let exit_state = $relay_state_construct(seed1.clone()).unwrap();

        let mut rng = rand::rng();
        let mut cell = [0u8; 509];
        rng.fill(&mut cell[..]);
        let cell: RelayBody = cell.into();
        (cell, exit_state)
    }};
}

/// Benchmark an exit encrypting a relay cell to send to the client.
/// This benches the encryption with all the originate logic.
pub fn client_encrypt_benchmark(c: &mut Criterion<impl Measurement>) {
    // Group for the Tor1 relay crypto with 498 bytes of data per relay cell.
    let mut group = c.benchmark_group("exit_encrypt");
    group.throughput(Throughput::Bytes(498));

    group.bench_function("Tor1RelayCrypto", |b| {
        b.iter_batched_ref(
            || exit_encrypt_setup!(tor1::Tor1RelayCryptState::<Aes128Ctr, Sha1>::construct),
            |(cell, relay_state)| {
                relay_state.originate(cell);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("Tor1Hsv3RelayCrypto", |b| {
        b.iter_batched_ref(
            || exit_encrypt_setup!(tor1::Tor1RelayCryptState::<Aes256Ctr, Sha3_256>::construct),
            |(cell, relay_state)| {
                relay_state.originate(cell);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();

    // Group for the Counter-Galois-Onion relay crypto with ~488 bytes of data per relay cell.
    let mut group = c.benchmark_group("exit_encrypt");
    group.throughput(Throughput::Bytes(488));

    #[cfg(feature = "counter-galois-onion")]
    group.bench_function("CGO_Aes128", |b| {
        b.iter_batched_ref(
            || exit_encrypt_setup!(cgo::CgoRelayCryptState::<Aes128Enc, Aes128Enc>::construct),
            |(cell, relay_state)| {
                relay_state.originate(cell);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    #[cfg(feature = "counter-galois-onion")]
    group.bench_function("CGO_Aes256", |b| {
        b.iter_batched_ref(
            || exit_encrypt_setup!(cgo::CgoRelayCryptState::<Aes256Enc, Aes256Enc>::construct),
            |(cell, relay_state)| {
                relay_state.originate(cell);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(
    name = client_encrypt;
    config = Criterion::default()
       .with_measurement(CyclesPerByte)
       .sample_size(5000);
    targets = client_encrypt_benchmark);
criterion_main!(client_encrypt);
