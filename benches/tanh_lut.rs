#![feature(once_cell)]

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use crate::tanh::TanhLut;

#[path = "../src/tanh.rs"]
mod tanh;

pub fn criterion_benchmark(c: &mut Criterion) {
    let xs = (0..360)
        .step_by(4)
        .map(|d| (d as f32).to_radians())
        .collect::<Vec<_>>();

    let mut group = c.benchmark_group("Tanh function");
    group.bench_function("trig", |b| {
        b.iter(|| {
            for x in xs.iter().copied() {
                let _ = black_box(x).tanh();
            }
        })
    });
    group.bench_function("lut", |b| {
        let lut = TanhLut::<false>::new();
        b.iter(|| {
            for x in xs.iter().copied() {
                lut.get(black_box(x));
            }
        })
    });
    group.bench_function("lut (lerp)", |b| {
        let lut = TanhLut::<true>::new();
        b.iter(|| {
            for x in xs.iter().copied() {
                lut.get(black_box(x));
            }
        })
    });
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
