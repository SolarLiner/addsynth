use std::simd::f32x8;

extern "C" {
    #[link_name = "llvm.cos.v8f32"]
    fn cos_v8f32(x: f32x8) -> f32x8;
    #[link_name = "llvm.sin.v8f32"]
    fn sin_v8f32(x: f32x8) -> f32x8;
}

pub trait SimdTrig<T, const LANES: usize> {
    fn sin(self) -> Self;
    fn cos(self) -> Self;
}

impl SimdTrig<f32, 8> for f32x8 {
    fn sin(self) -> Self {
        unsafe { sin_v8f32(self) }
    }

    fn cos(self) -> Self {
        unsafe { cos_v8f32(self) }
    }
}
