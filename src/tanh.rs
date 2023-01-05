#[derive(Debug, Clone)]
pub struct TanhLut<const LERP: bool> {
    values: [f32; 60],
}

impl<const LERP: bool> TanhLut<LERP> {
    pub fn new() -> Self {
        let mut values = [0.; 60];
        for i in 0..60 {
            let x = (i as f32 - 30.) / 10.;
            values[i] = x.tanh();
        }
        Self { values }
    }
}

impl TanhLut<true> {
    #[inline(always)]
    pub fn get(&self, x: f32) -> f32 {
        let x = (x + 3.).max(0.) * 10.;
        let i = (x.floor() as usize).min(self.values.len() - 2);
        let j = i + 1;
        let f = x.fract();
        unsafe {
            lerp(
                *self.values.get_unchecked(i),
                *self.values.get_unchecked(j),
                f,
            )
        }
    }
}

impl TanhLut<false> {
    #[inline(always)]
    pub fn get(&self, x: f32) -> f32 {
        let x = (x + 3.).max(0.) * 10.;
        let i = (x.floor() as usize).min(self.values.len() - 1);
        unsafe { *self.values.get_unchecked(i) }
    }
}

#[inline(always)]
fn lerp(x: f32, y: f32, t: f32) -> f32 {
    x + t * (y - x)
}

#[cfg(test)]
mod tests {

    #[test]
    fn reference_impl() {
        let lut = super::TanhLut::<true>::new();
        let xs = (-10..=10).map(|i| i as f32 / 3.3333333333333);
        let expected = xs.clone().map(f32::tanh).collect::<Vec<_>>();
        let actual = xs.map(|x| lut.get(x)).collect::<Vec<_>>();

        approx::assert_abs_diff_eq!(&*expected, &*actual, epsilon = 1e-2);
    }
}
