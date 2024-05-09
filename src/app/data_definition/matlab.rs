use anyhow::{anyhow, bail, Context};
use matio_rs::{Mat, MatArray, MatFile, MatioError, MayBeFrom};

use super::{DataLabel, DataPoint, DataPoints};

#[derive(Debug, Default)]
pub struct MatlabData {
    x: Vec<f64>,
    y: Vec<u8>,
}

#[allow(non_snake_case)]
impl MatlabData {
    fn new_zeroed(points_len: usize) -> Self {
        let result = Self {
            x: vec![0.0; points_len * 2],
            y: vec![0; points_len],
        };
        debug_assert!(result.validate().is_ok());
        result
    }

    fn X(&self) -> Result<Mat<'_>, MatioError> {
        let arr = MatArray::new(&self.x, vec![self.x.len() as u64 / 2, 2]);
        Mat::maybe_from("X", arr)
    }
    fn y(&self) -> Result<Mat<'_>, MatioError> {
        let arr = MatArray::new(&self.y, vec![self.y.len() as u64, 1]);
        Mat::maybe_from("y", arr)
    }

    pub fn save_to_file<P: AsRef<std::path::Path>>(&self, path: P) -> anyhow::Result<()> {
        let mat_file = matio_rs::MatFile::save(path)?;
        mat_file.write(
            self.X()
                .map_err(|e| anyhow!("matlab convert X failed with error: {e}"))?,
        );
        mat_file.write(
            self.y()
                .map_err(|e| anyhow!("matlab convert y failed with error: {e}"))?,
        );
        Ok(())
    }

    pub fn load_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<DataPoints, anyhow::Error> {
        let mat_file = MatFile::load(path)?;
        let x: Vec<f64> = mat_file.var("X")?;
        let y: Vec<u8> = match mat_file.var::<&str, Vec<u8>>("y") {
            Ok(val) => val,
            Err(e) => {
                if let MatioError::TypeMismatch(_var_name, _expected, found_type) = &e {
                    match &found_type[..] {
                        "DOUBLE" => mat_file
                            .var::<&str, Vec<f64>>("y")
                            .context("Error said to expect f64")?
                            .into_iter()
                            .map(|x| {
                                if x == 1.0 {
                                    Ok(1u8)
                                } else if x == 0.0 {
                                    Ok(0u8)
                                } else {
                                    bail!("Only expected 1 or 0 but found {x}")
                                }
                            })
                            .collect::<Result<Vec<_>, _>>()?,
                        "INT32" => mat_file
                            .var::<&str, Vec<i32>>("y")
                            .context("Error said to expect i32")?
                            .into_iter()
                            .map(|x| {
                                if x == 1 {
                                    Ok(1u8)
                                } else if x == 0 {
                                    Ok(0u8)
                                } else {
                                    bail!("Only expected 1 or 0 but found {x}")
                                }
                            })
                            .collect::<Result<Vec<_>, _>>()?,
                        "INT64" => mat_file
                            .var::<&str, Vec<i64>>("y")
                            .context("Error said to expect i64")?
                            .into_iter()
                            .map(|x| {
                                if x == 1 {
                                    Ok(1u8)
                                } else if x == 0 {
                                    Ok(0u8)
                                } else {
                                    bail!("Only expected 1 or 0 but found {x}")
                                }
                            })
                            .collect::<Result<Vec<_>, _>>()?,
                        _ => {
                            return Err(anyhow::Error::new(e)
                                .context("Currently Unsupported Type for \"y\""))
                        }
                    }
                } else {
                    return Err(e.into());
                }
            }
        };
        let loaded_data = Self { x, y };
        loaded_data.try_into()
    }

    /// Checks if the instance of Self is valid
    fn validate(&self) -> anyhow::Result<()> {
        if self.x.len() == self.y.len() * 2 {
            Ok(())
        } else {
            bail!(
                "validation failed. Expected 2 times the number of y values in X. But got {} values in y and {} in X but expected {} based on number in y. Does X have 2 columns?",
                self.y.len(), self.x.len(), self.y.len()*2
            )
        }
    }
}

impl From<&[DataPoint]> for MatlabData {
    fn from(points: &[DataPoint]) -> Self {
        let points_len = points.len();
        let mut result = Self::new_zeroed(points_len);
        for (i, point) in points.iter().enumerate() {
            result.x[i] = point.x0;
            result.x[i + points_len] = point.x1;
            result.y[i] = point.label.as_int() as _;
        }
        debug_assert!(result.validate().is_ok());
        result
    }
}

impl From<&DataPoints> for MatlabData {
    fn from(value: &DataPoints) -> Self {
        Self::from(value.as_slice())
    }
}

impl TryFrom<&MatlabData> for DataPoints {
    type Error = anyhow::Error;

    fn try_from(value: &MatlabData) -> Result<Self, Self::Error> {
        value.validate()?;
        let points_len = value.y.len();
        let mut result = Vec::with_capacity(points_len);
        for i in 0..points_len {
            result.push(DataPoint {
                x0: value.x[i],
                x1: value.x[i + points_len],
                label: DataLabel::try_from(value.y[i])
                    .context("unable to convert number to data label")?,
            });
        }
        Ok(result)
    }
}

impl TryFrom<MatlabData> for DataPoints {
    type Error = anyhow::Error;

    fn try_from(value: MatlabData) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::app::data_definition::{tests::generate_data_points, DataPoints};
    use pretty_assertions::assert_eq;

    #[test]
    fn conversion() {
        let original: DataPoints = generate_data_points();
        let converted: MatlabData = MatlabData::from(&original);
        let actual: DataPoints =
            DataPoints::try_from(&converted).expect("should be valid to convert back");
        assert_eq!(actual, original);
    }
}
