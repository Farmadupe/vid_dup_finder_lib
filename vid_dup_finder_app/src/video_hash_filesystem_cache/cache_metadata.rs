use cfg_if::cfg_if;
use std::str::FromStr;

use vid_dup_finder_lib::Cropdetect;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
enum OperatingSystem {
    Windows,
    Unix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseOperatingSystemError;
impl FromStr for OperatingSystem {
    type Err = ParseOperatingSystemError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "windows" => Ok(Self::Windows),
            "unix" => Ok(Self::Unix),
            _ => Err(ParseOperatingSystemError),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
enum DecodeBackend {
    FfmpegBackend,
    GstreamerBackend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseDecodeBackendError;
impl FromStr for DecodeBackend {
    type Err = ParseDecodeBackendError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "ffmpegbackend" => Ok(Self::FfmpegBackend),
            "gstreamerbackend" => Ok(Self::GstreamerBackend),
            _ => Err(ParseDecodeBackendError),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub(crate) struct VdfCacheMetadata {
    operating_system: OperatingSystem,
    decode_backend: DecodeBackend,
    crop: Cropdetect,
    skip_forward_amount: f64,
    cache_version: u64,
}

impl VdfCacheMetadata {
    pub fn new(crop: Cropdetect, skip_forward_amount: f64) -> Self {
        #[cfg(target_family = "windows")]
        let operating_system = OperatingSystem::Windows;

        #[cfg(target_family = "unix")]
        let operating_system = OperatingSystem::Unix;

        cfg_if! {
            if #[cfg(feature = "gstreamer_backend")] {
                let decode_backend = DecodeBackend::GstreamerBackend;
            } else  {
                let decode_backend = DecodeBackend::FfmpegBackend;
            }
        };

        let cache_version = 1;

        Self {
            operating_system,
            decode_backend,
            crop,
            skip_forward_amount,
            cache_version,
        }
    }

    pub fn to_disk_fmt(self) -> String {
        format!(
            "{:?},{:?},{:?},{},{}",
            self.operating_system,
            self.decode_backend,
            self.crop,
            self.skip_forward_amount,
            self.cache_version
        )
    }

    pub fn try_parse(val: &str) -> Result<Self, String> {
        let split = val.split([',']).collect::<Vec<_>>();

        match split[..] {
            [operating_system, decode_backend, crop, skip_forward_amount, cache_version] => {
                let operating_system =
                    OperatingSystem::from_str(operating_system).map_err(|_e| {
                        format!("Could not parse operating_system. Got {operating_system}")
                    })?;
                let decode_backend = DecodeBackend::from_str(decode_backend).map_err(|_e| {
                    format!("Could not parse decode_backend. Got {decode_backend}")
                })?;

                let crop = Cropdetect::from_str(crop)
                    .map_err(|_e| format!("Could not parse crop. Got {crop}"))?;

                let skip_forward_amount = skip_forward_amount.parse::<f64>().map_err(|_e| {
                    format!("Could not parse skip_forward amount. Got {skip_forward_amount}")
                })?;

                let cache_version = cache_version
                    .parse::<u64>()
                    .map_err(|_e| format!("Could not parse cache_version. Got {cache_version}"))?;

                Ok(Self {
                    operating_system,
                    decode_backend,
                    crop,
                    skip_forward_amount,
                    cache_version,
                })
            }
            _ => Err(format!("Could not parse cache metadata. Got {val}")),
        }
    }

    pub fn validate(
        self,
        exp_crop: Cropdetect,
        exp_skip_forward_amount: f64,
    ) -> Result<(), String> {
        let exp = Self::new(exp_crop, exp_skip_forward_amount);

        if self.operating_system != exp.operating_system {
            Err(format!(
                "operating_system mismatch: Act: {:?}, Exp: {:?}",
                self.operating_system, exp.operating_system
            ))
        } else if self.decode_backend != exp.decode_backend {
            Err(format!(
                "decode_backend mismatch: Act: {:?}, Exp: {:?}",
                self.decode_backend, exp.decode_backend
            ))
        } else if self.crop != exp.crop {
            Err(format!(
                "crop mismatch: Act: {:?}, Exp: {:?}",
                self.crop, exp.crop
            ))
        } else if self.skip_forward_amount != exp.skip_forward_amount {
            Err(format!(
                "skip_forward_amount mismatch: Act: {:?}, Exp: {:?}",
                self.skip_forward_amount, exp.skip_forward_amount
            ))
        } else if self.cache_version != exp.cache_version {
            Err(format!(
                "cache_version mismatch: Act: {:?}, Exp: {:?}",
                self.cache_version, exp.cache_version
            ))
        } else {
            Ok(())
        }
    }
}
