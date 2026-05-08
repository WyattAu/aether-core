//! Automatic mTLS Certificate Rotation
//!
//! Provides automatic certificate rotation for mesh nodes,
//! including certificate generation, distribution, and CRL updates.

use chrono::{DateTime, Utc};
use std::time::Duration;

/// Configuration for certificate rotation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CertRotationConfig {
    /// How often to check if rotation is needed
    pub check_interval: Duration,
    /// Rotate certificates this long before expiry
    pub rotate_before_expiry: Duration,
    /// Maximum certificate lifetime
    pub max_lifetime: Duration,
    /// CRL update interval
    pub crl_update_interval: Duration,
}

impl Default for CertRotationConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(3600),
            rotate_before_expiry: Duration::from_secs(24 * 60 * 60),
            max_lifetime: Duration::from_secs(7 * 24 * 60 * 60),
            crl_update_interval: Duration::from_secs(60),
        }
    }
}

/// Status of a certificate.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CertStatus {
    /// Certificate is valid and not near expiry
    Valid,
    /// Certificate is approaching expiry, rotation recommended
    ExpiringSoon,
    /// Certificate has expired
    Expired,
    /// Certificate has been revoked
    Revoked,
}

/// Information about a certificate.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CertInfo {
    /// Certificate serial number
    pub serial: String,
    /// Common name (node ID)
    pub common_name: String,
    /// Not-before timestamp
    pub not_before: DateTime<Utc>,
    /// Not-after timestamp
    pub not_after: DateTime<Utc>,
    /// Certificate status
    pub status: CertStatus,
    /// Issuer common name
    pub issuer: String,
    /// SHA-256 fingerprint
    pub fingerprint: String,
}

/// Certificate rotation state machine.
pub struct CertRotator {
    config: CertRotationConfig,
}

impl CertRotator {
    /// Create a new rotator with the given configuration.
    pub fn new(config: CertRotationConfig) -> Self {
        Self { config }
    }

    /// Determine if a certificate needs rotation based on its expiry time.
    pub fn needs_rotation(&self, not_after: DateTime<Utc>) -> bool {
        let now = Utc::now();
        let rotate_at = not_after
            - chrono::Duration::from_std(self.config.rotate_before_expiry)
                .unwrap_or(chrono::Duration::zero());
        now >= rotate_at
    }

    /// Get the status of a certificate based on its validity period.
    pub fn cert_status(&self, not_before: DateTime<Utc>, not_after: DateTime<Utc>) -> CertStatus {
        let now = Utc::now();
        if now < not_before {
            CertStatus::Valid
        } else if now >= not_after {
            CertStatus::Expired
        } else {
            let rotate_at = not_after
                - chrono::Duration::from_std(self.config.rotate_before_expiry)
                    .unwrap_or(chrono::Duration::zero());
            if now >= rotate_at {
                CertStatus::ExpiringSoon
            } else {
                CertStatus::Valid
            }
        }
    }

    /// Calculate the next check time.
    pub fn next_check_time(&self) -> DateTime<Utc> {
        Utc::now()
            + chrono::Duration::from_std(self.config.check_interval)
                .unwrap_or(chrono::Duration::zero())
    }

    /// Get the rotation config.
    pub fn config(&self) -> &CertRotationConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cert_status_valid() {
        let rotator = CertRotator::new(CertRotationConfig::default());
        let not_before = Utc::now() - chrono::Duration::days(1);
        let not_after = Utc::now() + chrono::Duration::days(5);
        assert_eq!(
            rotator.cert_status(not_before, not_after),
            CertStatus::Valid
        );
    }

    #[test]
    fn test_cert_status_expiring() {
        let rotator = CertRotator::new(CertRotationConfig {
            rotate_before_expiry: Duration::from_secs(2 * 24 * 60 * 60),
            ..CertRotationConfig::default()
        });
        let not_before = Utc::now() - chrono::Duration::days(5);
        let not_after = Utc::now() + chrono::Duration::hours(12);
        assert_eq!(
            rotator.cert_status(not_before, not_after),
            CertStatus::ExpiringSoon
        );
    }

    #[test]
    fn test_cert_status_expired() {
        let rotator = CertRotator::new(CertRotationConfig::default());
        let not_before = Utc::now() - chrono::Duration::days(10);
        let not_after = Utc::now() - chrono::Duration::days(1);
        assert_eq!(
            rotator.cert_status(not_before, not_after),
            CertStatus::Expired
        );
    }

    #[test]
    fn test_needs_rotation_false() {
        let rotator = CertRotator::new(CertRotationConfig {
            rotate_before_expiry: Duration::from_secs(24 * 60 * 60),
            ..CertRotationConfig::default()
        });
        let not_after = Utc::now() + chrono::Duration::days(5);
        assert!(!rotator.needs_rotation(not_after));
    }

    #[test]
    fn test_needs_rotation_true() {
        let rotator = CertRotator::new(CertRotationConfig {
            rotate_before_expiry: Duration::from_secs(2 * 24 * 60 * 60),
            ..CertRotationConfig::default()
        });
        let not_after = Utc::now() + chrono::Duration::hours(12);
        assert!(rotator.needs_rotation(not_after));
    }

    #[test]
    fn test_next_check_time() {
        let rotator = CertRotator::new(CertRotationConfig {
            check_interval: Duration::from_secs(3600),
            ..CertRotationConfig::default()
        });
        let next = rotator.next_check_time();
        let expected = Utc::now() + chrono::Duration::seconds(3600);
        let diff = (next.timestamp() - expected.timestamp()).abs();
        assert!(diff <= 2, "next check time should be ~1 hour from now");
    }

    #[test]
    fn test_cert_info_serialization() {
        let info = CertInfo {
            serial: "12345".to_string(),
            common_name: "node-1".to_string(),
            not_before: Utc::now(),
            not_after: Utc::now() + chrono::Duration::days(7),
            status: CertStatus::Valid,
            issuer: "aether-ca".to_string(),
            fingerprint: "abcd1234".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let deserialized: CertInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info.serial, deserialized.serial);
        assert_eq!(info.common_name, deserialized.common_name);
    }

    #[test]
    fn test_config_defaults() {
        let config = CertRotationConfig::default();
        assert_eq!(config.check_interval, Duration::from_secs(3600));
        assert_eq!(config.max_lifetime, Duration::from_secs(7 * 24 * 60 * 60));
    }
}
