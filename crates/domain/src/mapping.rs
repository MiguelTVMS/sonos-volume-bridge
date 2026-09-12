use crate::{NormalizedVolume, SonosVolume};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MappingPoint {
    pub local: NormalizedVolume,
    pub sonos: SonosVolume,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VolumeMapping {
    Linear,
    CappedLinear { maximum: SonosVolume },
    Piecewise { points: Vec<MappingPoint> },
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum MappingError {
    #[error("a piecewise mapping requires at least two points")]
    TooFewPoints,
    #[error("mapping points must start at local 0 and end at local 100")]
    MissingEndpoints,
    #[error("mapping points must strictly increase in local and not decrease in Sonos volume")]
    NotMonotonic,
}

impl VolumeMapping {
    pub fn validate(&self) -> Result<(), MappingError> {
        let Self::Piecewise { points } = self else {
            return Ok(());
        };
        if points.len() < 2 {
            return Err(MappingError::TooFewPoints);
        }
        if points
            .first()
            .is_none_or(|p| p.local != NormalizedVolume::MIN)
            || points
                .last()
                .is_none_or(|p| p.local != NormalizedVolume::MAX)
        {
            return Err(MappingError::MissingEndpoints);
        }
        if points
            .windows(2)
            .any(|p| p[0].local >= p[1].local || p[0].sonos > p[1].sonos)
        {
            return Err(MappingError::NotMonotonic);
        }
        Ok(())
    }

    pub fn to_sonos(
        &self,
        local: NormalizedVolume,
        cap: SonosVolume,
    ) -> Result<SonosVolume, MappingError> {
        self.validate()?;
        let value = match self {
            Self::Linear => local.get(),
            Self::CappedLinear { maximum } => scale(local.get(), maximum.get()),
            Self::Piecewise { points } => interpolate_forward(points, local.get()),
        };
        Ok(SonosVolume::new(value)
            .expect("interpolation is bounded")
            .capped_at(cap))
    }

    pub fn to_local(&self, sonos: SonosVolume) -> Result<NormalizedVolume, MappingError> {
        self.validate()?;
        let value = match self {
            Self::Linear => sonos.get(),
            Self::CappedLinear { maximum } => {
                reverse_scale(sonos.get().min(maximum.get()), maximum.get())
            }
            Self::Piecewise { points } => interpolate_reverse(points, sonos.get()),
        };
        NormalizedVolume::new(value).map_err(|_| MappingError::NotMonotonic)
    }
}

fn bounded_u8(value: i32) -> u8 {
    // Callers only use validated 0..=100 endpoints, so this conversion is total.
    u8::try_from(value).expect("interpolation result is in the u8 range")
}
fn scale(value: u8, maximum: u8) -> u8 {
    bounded_u8(i32::from(
        (u16::from(value) * u16::from(maximum) + 50) / 100,
    ))
}
fn reverse_scale(value: u8, maximum: u8) -> u8 {
    if maximum == 0 {
        0
    } else {
        bounded_u8(i32::from(
            (u16::from(value) * 100 + u16::from(maximum) / 2) / u16::from(maximum),
        ))
    }
}
fn lerp(x: u8, x0: u8, y0: u8, x1: u8, y1: u8) -> u8 {
    bounded_u8(
        i32::from(y0)
            + (i32::from(x - x0) * (i32::from(y1) - i32::from(y0)) + i32::from(x1 - x0) / 2)
                / i32::from(x1 - x0),
    )
}
fn interpolate_forward(points: &[MappingPoint], local: u8) -> u8 {
    let segment = points
        .windows(2)
        .find(|p| local <= p[1].local.get())
        .expect("validated endpoints");
    lerp(
        local,
        segment[0].local.get(),
        segment[0].sonos.get(),
        segment[1].local.get(),
        segment[1].sonos.get(),
    )
}
fn interpolate_reverse(points: &[MappingPoint], sonos: u8) -> u8 {
    if sonos <= points[0].sonos.get() {
        return points[0].local.get();
    }
    let segment = points.windows(2).find(|p| sonos <= p[1].sonos.get());
    match segment {
        Some(p) if p[0].sonos == p[1].sonos => p[1].local.get(),
        Some(p) => lerp(
            sonos,
            p[0].sonos.get(),
            p[0].local.get(),
            p[1].sonos.get(),
            p[1].local.get(),
        ),
        None => points.last().expect("validated points").local.get(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn v(value: u8) -> NormalizedVolume {
        NormalizedVolume::new(value).unwrap()
    }
    fn s(value: u8) -> SonosVolume {
        SonosVolume::new(value).unwrap()
    }
    #[test]
    fn linear_endpoints_and_cap() {
        assert_eq!(VolumeMapping::Linear.to_sonos(v(100), s(55)), Ok(s(55)));
        assert_eq!(VolumeMapping::Linear.to_local(s(33)), Ok(v(33)));
    }
    #[test]
    fn capped_linear_round_trip() {
        let map = VolumeMapping::CappedLinear { maximum: s(55) };
        assert_eq!(map.to_sonos(v(60), s(100)), Ok(s(33)));
        assert_eq!(map.to_local(s(33)), Ok(v(60)));
    }
    #[test]
    fn interpolates_piecewise() {
        let map = VolumeMapping::Piecewise {
            points: vec![
                MappingPoint {
                    local: v(0),
                    sonos: s(0),
                },
                MappingPoint {
                    local: v(40),
                    sonos: s(12),
                },
                MappingPoint {
                    local: v(100),
                    sonos: s(55),
                },
            ],
        };
        assert_eq!(map.to_sonos(v(20), s(100)), Ok(s(6)));
        assert_eq!(map.to_local(s(34)), Ok(v(71)));
    }
    #[test]
    fn rejects_invalid_curve() {
        let map = VolumeMapping::Piecewise {
            points: vec![
                MappingPoint {
                    local: v(0),
                    sonos: s(5),
                },
                MappingPoint {
                    local: v(100),
                    sonos: s(0),
                },
            ],
        };
        assert_eq!(map.validate(), Err(MappingError::NotMonotonic));
    }
}
