
pub type Degrees = f32;
pub type AzAngle = Degrees;
pub type ElAngle = Degrees;

#[derive(Default, Debug, Copy, Clone)]
pub struct Pos {
    pub az: AzAngle,
    pub el: ElAngle,
}

impl std::ops::Sub for Pos {
    type Output = Self;

    fn sub(self, other: Self) -> Self::Output {
        Self {
            az: self.az - other.az,
            el: self.el - other.el,
        }
    }
}

impl std::ops::Add for Pos {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        Self {
            az: self.az + other.az,
            el: self.el + other.el,
        }
    }
}


// HT: https://stackoverflow.com/a/34969944/179583
pub(crate) fn parse_pair<'a, I>(mut parts: I) -> Option<Pos>
where
    I: Iterator<Item = &'a str>,
{
    let az_part = parts.next()?;
    let el_part = parts.next()?;

    let az: Degrees = az_part.parse().ok()?;
    let el: Degrees = el_part.parse().ok()?;
    return Some(Pos { az, el });
}


pub fn predict_rotation(curr: AzAngle, dest: AzAngle) -> Degrees {
    let naïve = dest - curr;
    let rotation = naïve % 360.0;
    if rotation > 180.0 {
        rotation - 360.0
    } else if rotation < -180.0 {
        rotation + 360.0
    } else {
        rotation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_rotations() {
        assert_eq!(
            predict_rotation(5.0, 10.0), 5.0,
        );
        assert_eq!(
            predict_rotation(10.0, 5.0), -5.0,
        );
        assert_eq!(
            predict_rotation(365.0, 370.0), 5.0,
        );
        assert_eq!(
            predict_rotation(365.0, 5.0), 0.0,
        );
        assert_eq!(
            predict_rotation(365.0, -355.0), 0.0,
        );
        assert_eq!(
            predict_rotation(0.0, 185.0), -175.0,
        );
        assert_eq!(
            predict_rotation(0.0, -185.0), 175.0,
        );
        assert_eq!(
            predict_rotation(0.0, -175.0), -175.0,
        );
        assert_eq!(
            predict_rotation(360.0, 175.0), 175.0,
        );
        assert_eq!(
            predict_rotation(-123456.0, 42.0), 18.0,
        );
        assert_eq!(
            predict_rotation(-123456.0, -123467.0), -11.0,
        );
        assert_eq!(
            predict_rotation(-123456.0, 123186.0), 42.0,
        );
    }
}

pub fn recommended_midpoint(
    curr: AzAngle,
    diff: &mut Degrees,
    max_mag: AzAngle,
) -> Option<AzAngle> {
    let past_max = curr + *diff > max_mag;
    let past_min = curr + *diff < -max_mag;
    if past_max || past_min {
        // find an intermediate 180º from halfway to dest
        // e.g. we're at MAX_WINDING=360 and diff is +90:
        // halfway is 405°, whose antipode is (45 ± 180)°
        // if we go there, next diff should then unwind.
        let antimid = curr + *diff / 2.0 + 180.0;
        // adjust to newly intended (unwinding direction) difference
        *diff = if past_max { *diff - 360.0 } else { *diff + 360.0 };
        Some(antimid)
    } else {
        None
    }
}


// HT: https://internals.rust-lang.org/t/why-is-there-no-tryclone-trait/19977
pub(crate) trait TryClone: Sized {
    fn try_clone(&self) -> std::io::Result<Self>;
}

impl TryClone for std::fs::File {
    fn try_clone(&self) -> std::io::Result<Self> {
        // Resolves to the inherent `File::try_clone`, not this trait method,
        // because inherent methods take priority over trait methods.
        Self::try_clone(self)
    }
}

impl TryClone for std::net::TcpStream {
    fn try_clone(&self) -> std::io::Result<Self> {
        // Resolves to the inherent `File::try_clone`, not this trait method,
        // because inherent methods take priority over trait methods.
        Self::try_clone(self)
    }
}
