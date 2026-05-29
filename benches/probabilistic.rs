//! Microbenchmarks for the hot paths of every structure.
//!
//! Run with `cargo bench --bench probabilistic`. Each group measures the
//! steady-state per-operation cost on a pre-populated structure, since that is
//! the cost that dominates a streaming workload.

use bloom_lib::{BloomFilter, CountMinSketch, CuckooFilter, HyperLogLog, MinHash, TopK};
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_bloom(c: &mut Criterion) {
    let mut group = c.benchmark_group("bloom");

    group.bench_function("insert", |b| {
        let mut filter = BloomFilter::new(100_000, 0.01).unwrap();
        let mut i = 0u64;
        b.iter(|| {
            i = i.wrapping_add(1);
            black_box(filter.insert(black_box(&i)))
        });
    });

    group.bench_function("contains_hit", |b| {
        let mut filter = BloomFilter::new(100_000, 0.01).unwrap();
        for i in 0..50_000u64 {
            let _ = filter.insert(&i);
        }
        let mut i = 0u64;
        b.iter(|| {
            i = (i + 1) % 50_000;
            black_box(filter.contains(black_box(&i)))
        });
    });

    group.finish();
}

fn bench_cuckoo(c: &mut Criterion) {
    let mut group = c.benchmark_group("cuckoo");

    group.bench_function("contains_hit", |b| {
        let mut filter = CuckooFilter::new(100_000).unwrap();
        for i in 0..50_000u64 {
            let _ = filter.insert(&i);
        }
        let mut i = 0u64;
        b.iter(|| {
            i = (i + 1) % 50_000;
            black_box(filter.contains(black_box(&i)))
        });
    });

    group.finish();
}

fn bench_count_min(c: &mut Criterion) {
    let mut group = c.benchmark_group("count_min");

    group.bench_function("increment", |b| {
        let mut sketch = CountMinSketch::new(0.001, 0.001).unwrap();
        let mut i = 0u64;
        b.iter(|| {
            i = i.wrapping_add(1);
            sketch.increment(black_box(&i));
        });
    });

    group.bench_function("estimate", |b| {
        let mut sketch = CountMinSketch::new(0.001, 0.001).unwrap();
        for i in 0..50_000u64 {
            sketch.increment(&i);
        }
        let mut i = 0u64;
        b.iter(|| {
            i = (i + 1) % 50_000;
            black_box(sketch.estimate(black_box(&i)))
        });
    });

    group.finish();
}

fn bench_hyperloglog(c: &mut Criterion) {
    c.bench_function("hyperloglog_insert", |b| {
        let mut hll = HyperLogLog::new(14).unwrap();
        let mut i = 0u64;
        b.iter(|| {
            i = i.wrapping_add(1);
            hll.insert(black_box(&i));
        });
    });
}

fn bench_minhash(c: &mut Criterion) {
    c.bench_function("minhash_insert_128", |b| {
        let mut sketch = MinHash::new(128).unwrap();
        let mut i = 0u64;
        b.iter(|| {
            i = i.wrapping_add(1);
            sketch.insert(black_box(&i));
        });
    });
}

fn bench_topk(c: &mut Criterion) {
    c.bench_function("topk_insert", |b| {
        let mut top = TopK::new(100, 0.001, 0.001).unwrap();
        let mut i = 0u64;
        b.iter(|| {
            i = i.wrapping_add(1);
            top.insert(black_box(i));
        });
    });
}

criterion_group!(
    benches,
    bench_bloom,
    bench_cuckoo,
    bench_count_min,
    bench_hyperloglog,
    bench_minhash,
    bench_topk,
);
criterion_main!(benches);
