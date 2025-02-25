use crate::series::ops::SeriesSealed;
use polars_core::export::num;
use polars_core::export::num::{Float, FromPrimitive};
use polars_core::prelude::*;
use polars_core::utils::with_unstable_series;
use std::ops::SubAssign;

#[cfg(feature = "moment")]
fn rolling_skew<T>(ca: &ChunkedArray<T>, window_size: usize, bias: bool) -> Result<ChunkedArray<T>>
where
    ChunkedArray<T>: IntoSeries,
    T: PolarsFloatType,
    T::Native: Float + IsFloat + SubAssign + num::pow::Pow<T::Native, Output = T::Native>,
{
    with_unstable_series(ca.dtype(), |us| {
        ca.rolling_apply_float(window_size, |arr| {
            let arr = unsafe { arr.chunks_mut().get_mut(0).unwrap() };

            us.with_array(arr, |us| {
                us.as_ref()
                    .skew(bias)
                    .unwrap()
                    .map(|flt| T::Native::from_f64(flt).unwrap())
            })
        })
    })
}

pub trait RollingSeries: SeriesSealed {
    #[cfg(feature = "moment")]
    fn rolling_skew(&self, window_size: usize, bias: bool) -> Result<Series> {
        let s = self.as_series();

        match s.dtype() {
            DataType::Float64 => {
                let ca = s.f64().unwrap();
                rolling_skew(ca, window_size, bias).map(|ca| ca.into_series())
            }
            DataType::Float32 => {
                let ca = s.f32().unwrap();
                rolling_skew(ca, window_size, bias).map(|ca| ca.into_series())
            }
            dt if dt.is_numeric() => {
                let s = s.cast(&DataType::Float64).unwrap();
                s.rolling_skew(window_size, bias)
            }
            dt => Err(PolarsError::ComputeError(
                format!(
                    "cannot use rolling_skew function on Series of dtype: {:?}",
                    dt
                )
                .into(),
            )),
        }
    }
}

impl RollingSeries for Series {}
