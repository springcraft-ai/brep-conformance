//! Maths that gives the same bits on every platform. The standard library's `sin`, `cos` and the
//! rest call each platform's own maths library, whose last bits differ; a case drawn with them, and
//! any branch taken on them, can differ between platforms, and with it the case's id. Code that
//! draws cases, or works out what they hold, uses these instead. `sqrt` is exactly rounded
//! everywhere and needs none.

pub trait Portable {
    fn psin(self) -> f64;
    fn pcos(self) -> f64;
    fn ptan(self) -> f64;
    fn pasin(self) -> f64;
    fn pacos(self) -> f64;
    fn patan(self) -> f64;
    fn patan2(self, x: f64) -> f64;
    fn psin_cos(self) -> (f64, f64);
    fn pexp(self) -> f64;
    fn pcbrt(self) -> f64;
    fn ppowf(self, e: f64) -> f64;
    fn ppowi(self, n: i32) -> f64;
    fn phypot(self, y: f64) -> f64;
}

impl Portable for f64 {
    fn psin(self) -> f64 {
        libm::sin(self)
    }
    fn pcos(self) -> f64 {
        libm::cos(self)
    }
    fn ptan(self) -> f64 {
        libm::tan(self)
    }
    fn pasin(self) -> f64 {
        libm::asin(self)
    }
    fn pacos(self) -> f64 {
        libm::acos(self)
    }
    fn patan(self) -> f64 {
        libm::atan(self)
    }
    fn patan2(self, x: f64) -> f64 {
        libm::atan2(self, x)
    }
    fn psin_cos(self) -> (f64, f64) {
        (libm::sin(self), libm::cos(self))
    }
    fn pexp(self) -> f64 {
        libm::exp(self)
    }
    fn pcbrt(self) -> f64 {
        libm::cbrt(self)
    }
    fn ppowf(self, e: f64) -> f64 {
        libm::pow(self, e)
    }
    // By repeated multiplication in a fixed order: the standard `powi` is left to the compiler,
    // which may order the products differently on another target.
    fn ppowi(self, n: i32) -> f64 {
        let mut out = 1.0;
        for _ in 0..n.unsigned_abs() {
            out *= self;
        }
        if n < 0 { 1.0 / out } else { out }
    }
    fn phypot(self, y: f64) -> f64 {
        libm::hypot(self, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agrees_with_the_standard_library_to_within_its_last_bits() {
        for &x in &[0.1, 0.5, 1.0, 2.5, -3.0] {
            assert!((x.psin() - x.sin()).abs() < 1e-15);
            assert!((x.pcos() - x.cos()).abs() < 1e-15);
            assert!((x.patan2(0.7) - x.atan2(0.7)).abs() < 1e-15);
        }
        assert_eq!(1.5_f64.ppowi(3), 3.375);
        assert_eq!(2.0_f64.ppowi(-2), 0.25);
        assert_eq!(3.0_f64.phypot(4.0), 5.0);
    }
}
