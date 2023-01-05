use std::{marker::PhantomData, ops};

pub const fn autodiff<
    T: Copy + ops::Add<Output = T> + ops::Sub<Output = T> + ops::Div<Output = T>,
    F: Fn(T) -> T,
>(
    delta: T,
    func: F,
) -> impl Fn(T) -> T {
    move |x| (func(x + delta) - func(x)) / delta
}

pub const fn make_function<
    'f,
    T: 'f + Copy + ops::Add<Output = T> + ops::Sub<Output = T> + ops::Div<Output = T>,
    F: 'f + Fn(T) -> T,
>(
    ad_delta: T,
    func: &'f F,
) -> Function<T, &'f F, impl 'f + Fn(T) -> T> {
    let diff = autodiff(ad_delta, func);
    Function {
        func,
        diff,
        __phantom: PhantomData,
    }
}

pub struct Function<T, F, D> {
    func: F,
    diff: D,
    __phantom: PhantomData<fn(T) -> T>,
}

impl<
        T: Copy + ops::Add<Output = T> + ops::Sub<Output = T> + ops::Div<Output = T>,
        F: Fn(T) -> T,
        D: Fn(T) -> T,
    > Function<T, F, D>
{
    pub const fn new(func: F, diff: D) -> Self {
        Self {
            func,
            diff,
            __phantom: PhantomData,
        }
    }

    pub fn solve(&self, iterations: usize, initial_guess: T) -> T {
        (0..iterations).fold(initial_guess, |guess, _| self.step(guess))
    }

    fn step(&self, guess: T) -> T {
        guess - (self.func)(guess) / (self.diff)(guess)
    }
}
