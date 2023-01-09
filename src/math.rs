use nalgebra::{ComplexField, SMatrix, SVector, Scalar};
use num_traits::Zero;

pub trait ScalarField<T, const N: usize> {
    fn eval(&self, x: &SVector<T, N>) -> SVector<T, N>;
    fn jacobian(&self, x: &SVector<T, N>) -> SMatrix<T, N, N>;
}

pub fn nr_step<T: ComplexField + Scalar, S, const N: usize>(
    s: &S,
    x: &SVector<T, N>,
) -> Option<SVector<T, N>>
where
    S: ScalarField<T, N>,
{
    let j = s.jacobian(x);
    let Some(ij) = j.clone_owned().try_inverse() else {
        eprintln!("Jacobian matrix non-invertible:\n{j}");
        return None;
    };
    Some(ij * s.eval(x))
}

pub trait Differential<T, const N: usize> {
    fn dv(&self, t: T, yprev: &SVector<T, N>) -> SVector<T, N>;
}

impl<'a, T, D, const N: usize> Differential<T, N> for &'a D
where
    D: Differential<T, N>,
{
    fn dv(&self, t: T, yprev: &SVector<T, N>) -> SVector<T, N> {
        D::dv(self, t, yprev)
    }
}

pub fn trapezoidal_step<const N: usize>(
    diff: impl Differential<f32, N>,
    prev: &SVector<f32, N>,
    t: f32,
    dt: f32,
) -> SVector<f32, N> {
    let s = prev + diff.dv(t, prev);
    s * dt / 2.
}

#[cfg(test)]
mod tests {
    use approx::assert_abs_diff_eq;
    use nalgebra::{SMatrix, SVector};
    use crate::math::{nr_step, ScalarField};

    #[test]
    fn newton_rhapson_single() {
        struct ToSolve;

        impl ScalarField<f64, 1> for ToSolve {
            fn eval(&self, x: &SVector<f64, 1>) -> SVector<f64, 1> {
                x.map(|x| x.tanh() - (5. - x))
            }

            fn jacobian(&self, x: &SVector<f64, 1>) -> SMatrix<f64, 1, 1> {
                let x = x[0];
                SMatrix::<_, 1, 1>::new(2. - x.tanh().powi(2))
            }
        }

        let mut x = SVector::<f64, 1>::new(0.);
        for _ in 0..10 {
            x -= nr_step(&ToSolve, &x).unwrap();
            println!("{x}");
        }
        assert_abs_diff_eq!(4., x[0], epsilon=1e-3);
    }

    #[test]
    fn newton_rhapson_multi() {
        struct ToSolve;

        impl ScalarField<f64, 2> for ToSolve {
            fn eval(&self, x: &SVector<f64, 2>) -> SVector<f64, 2> {
                SVector::<_, 2>::new(
                    x[0].ln() - 1.,
                    -x[1].ln(),
                )
            }

            fn jacobian(&self, x: &SVector<f64, 2>) -> SMatrix<f64, 2, 2> {
                SMatrix::from_diagonal(&SVector::<_, 2>::new(x[0], -x[1]).map(|x| x.recip()))
            }
        }

        let mut x = SVector::<f64, 2>::new(0.5, 0.5);
        for _ in 0..10 {
            x -= nr_step(&ToSolve, &x).unwrap();
            println!("{x}");
        }
        assert_abs_diff_eq!(std::f64::consts::E, x[0], epsilon=1e-3);
        assert_abs_diff_eq!(1., x[1], epsilon=1e-3);
    }
}