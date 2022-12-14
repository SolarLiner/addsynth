use criterion::{black_box, criterion_group, criterion_main, Criterion};

struct SineLut<const Lerp: bool> {
    values: Vec<f32>,
}

impl<const Lerp: bool> SineLut<Lerp> {
    pub fn new() -> Self {
        let values = (0..360).map(|d| (d as f32).to_radians().sin()).collect();
        Self { values }
    }
}

impl SineLut<true> {
    #[inline(always)]
    pub fn get(&self, x: f32) -> f32 {
        let x = x.to_degrees() + 360.;
        let i = x.floor() as usize % 360;
        let j = (i + 1) % 360;
        let f = x.fract();
        lerp(self.values[i], self.values[j], f)
    }
}

impl SineLut<false> {
    #[inline(always)]
    pub fn get(&self, x: f32) -> f32 {
        let x = x.to_degrees() + 360.;
        let i = x.floor() as usize % 360;
        self.values[i]
    }
}

#[inline(always)]
fn lerp(x: f32, y: f32, t: f32) -> f32 {
    x + (1. - t) * (y - x)
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let xs = (0..360)
        .step_by(4)
        .map(|d| (d as f32).to_radians())
        .collect::<Vec<_>>();
    let lut = SineLut::<false>::new();
    let lut_lerp = SineLut::<true>::new();

    let mut group = c.benchmark_group("Sine function");
    group.bench_function("sine", |b| {
        b.iter(|| {
            for x in xs.iter().copied() {
                let _ = black_box(x).sin();
            }
        })
    });
    group.bench_function("lut", |b| {
        b.iter(|| {
            for x in xs.iter().copied() {
                lut.get(black_box(x));
            }
        })
    });
    group.bench_function("lut (lerp)", |b| {
        b.iter(|| {
            for x in xs.iter().copied() {
                lut_lerp.get(black_box(x));
            }
        })
    });
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
