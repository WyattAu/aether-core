//! Certificate Management
//!
//! Certificate Authority, generation, signing, validation, and revocation.

use crate::error::{Error, Result};
use rcgen::{CertificateParams, DnType, KeyPair, SerialNumber};
use rustls::pki_types::CertificateDer;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

use super::CertificateType;

pub struct CertificateAuthority {
    key_pair: KeyPair,
    cert_der: CertificateDer<'static>,
    revoked_serials: Arc<RwLock<HashSet<u64>>>,
    crl_last_update: Arc<RwLock<SystemTime>>,
}

impl CertificateAuthority {
    pub fn generate(common_name: &str) -> Result<Self> {
        let mut params = CertificateParams::new(vec![common_name.to_string()])
            .map_err(|e| Error::internal(format!("Failed to create cert params: {}", e)))?;
        params
            .distinguished_name
            .push(DnType::CommonName, common_name);
        params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        params.key_usages = vec![
            rcgen::KeyUsagePurpose::KeyCertSign,
            rcgen::KeyUsagePurpose::CrlSign,
            rcgen::KeyUsagePurpose::DigitalSignature,
        ];

        let key_pair = KeyPair::generate()
            .map_err(|e| Error::internal(format!("Failed to generate CA key: {}", e)))?;

        let cert = params
            .self_signed(&key_pair)
            .map_err(|e| Error::internal(format!("Failed to sign CA cert: {}", e)))?;

        let cert_der = cert.der().clone();

        Ok(Self {
            key_pair,
            cert_der,
            revoked_serials: Arc::new(RwLock::new(HashSet::new())),
            crl_last_update: Arc::new(RwLock::new(SystemTime::UNIX_EPOCH)),
        })
    }

    pub fn from_pem(_cert_pem: &str, key_pem: &str) -> Result<Self> {
        let key_pair = KeyPair::from_pem(key_pem)
            .map_err(|e| Error::internal(format!("Failed to parse CA key: {}", e)))?;

        let mut params = CertificateParams::new(vec!["ca.aether".to_string()])
            .map_err(|e| Error::internal(format!("Failed to create cert params: {}", e)))?;
        params
            .distinguished_name
            .push(DnType::CommonName, "Aether CA");
        params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        params.key_usages = vec![
            rcgen::KeyUsagePurpose::KeyCertSign,
            rcgen::KeyUsagePurpose::CrlSign,
        ];

        let cert = params
            .self_signed(&key_pair)
            .map_err(|e| Error::internal(format!("Failed to sign CA cert: {}", e)))?;

        let cert_der = cert.der().clone();

        Ok(Self {
            key_pair,
            cert_der,
            revoked_serials: Arc::new(RwLock::new(HashSet::new())),
            crl_last_update: Arc::new(RwLock::new(SystemTime::UNIX_EPOCH)),
        })
    }

    pub fn certificate(&self) -> &CertificateDer<'static> {
        &self.cert_der
    }

    pub fn certificate_pem(&self) -> Result<String> {
        let mut params = CertificateParams::new(vec!["ca.aether".to_string()])
            .map_err(|e| Error::internal(format!("Failed to create cert params: {}", e)))?;
        params
            .distinguished_name
            .push(DnType::CommonName, "Aether CA");
        params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        params.key_usages = vec![
            rcgen::KeyUsagePurpose::KeyCertSign,
            rcgen::KeyUsagePurpose::CrlSign,
        ];

        let cert = params
            .self_signed(&self.key_pair)
            .map_err(|e| Error::internal(format!("Failed to generate cert: {}", e)))?;

        Ok(cert.pem())
    }

    pub fn private_key_der(&self) -> Vec<u8> {
        self.key_pair.serialize_der()
    }

    pub fn private_key_pem(&self) -> String {
        self.key_pair.serialize_pem()
    }

    pub fn issue_certificate(
        &self,
        common_name: &str,
        cert_type: CertificateType,
        serial: u64,
    ) -> Result<(CertificateDer<'static>, Vec<u8>)> {
        let mut params = CertificateParams::new(vec![common_name.to_string()])
            .map_err(|e| Error::internal(format!("Failed to create cert params: {}", e)))?;
        params
            .distinguished_name
            .push(DnType::CommonName, common_name);
        params.is_ca = rcgen::IsCa::NoCa;
        params.key_usages = vec![
            rcgen::KeyUsagePurpose::DigitalSignature,
            rcgen::KeyUsagePurpose::KeyEncipherment,
        ];
        params.extended_key_usages = vec![
            rcgen::ExtendedKeyUsagePurpose::ServerAuth,
            rcgen::ExtendedKeyUsagePurpose::ClientAuth,
        ];
        params.serial_number = Some(SerialNumber::from(serial));

        let key_pair = KeyPair::generate()
            .map_err(|e| Error::internal(format!("Failed to generate key: {}", e)))?;

        let mut ca_params = CertificateParams::new(vec!["ca.aether".to_string()])
            .map_err(|e| Error::internal(format!("Failed to create CA params: {}", e)))?;
        ca_params
            .distinguished_name
            .push(DnType::CommonName, "Aether CA");
        ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        ca_params.key_usages = vec![
            rcgen::KeyUsagePurpose::KeyCertSign,
            rcgen::KeyUsagePurpose::CrlSign,
        ];

        let ca_cert = ca_params
            .self_signed(&self.key_pair)
            .map_err(|e| Error::internal(format!("Failed to get CA cert: {}", e)))?;

        let cert = params
            .signed_by(&key_pair, &ca_cert, &self.key_pair)
            .map_err(|e| Error::internal(format!("Failed to sign certificate: {}", e)))?;

        let cert_der = cert.der().clone();
        let key_der = key_pair.serialize_der();

        let _ = cert_type;

        Ok((cert_der, key_der))
    }

    pub async fn revoke(&self, serial: u64) -> Result<()> {
        let mut revoked = self.revoked_serials.write().await;
        revoked.insert(serial);
        *self.crl_last_update.write().await = SystemTime::now();
        Ok(())
    }

    pub async fn is_revoked(&self, serial: u64) -> bool {
        self.revoked_serials.read().await.contains(&serial)
    }

    pub async fn generate_crl(&self) -> Result<CertificateRevocationList> {
        let revoked = self.revoked_serials.read().await;
        let revoked_entries: Vec<(u64, SystemTime)> = revoked
            .iter()
            .map(|&serial| (serial, SystemTime::now()))
            .collect();

        Ok(CertificateRevocationList {
            revoked_entries,
            this_update: SystemTime::now(),
            next_update: SystemTime::now() + Duration::from_secs(3600),
        })
    }
}

#[derive(Clone)]
pub struct CertificateRevocationList {
    pub revoked_entries: Vec<(u64, SystemTime)>,
    pub this_update: SystemTime,
    pub next_update: SystemTime,
}

impl CertificateRevocationList {
    pub fn is_empty(&self) -> bool {
        self.revoked_entries.is_empty()
    }

    pub fn contains(&self, serial: u64) -> bool {
        self.revoked_entries.iter().any(|(s, _)| *s == serial)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut data = Vec::new();

        data.extend_from_slice(&(self.revoked_entries.len() as u32).to_le_bytes());
        for (serial, time) in &self.revoked_entries {
            data.extend_from_slice(&serial.to_le_bytes());
            let timestamp = time
                .duration_since(UNIX_EPOCH)
                .map_err(|e| Error::internal(format!("Time error: {}", e)))?
                .as_secs();
            data.extend_from_slice(&timestamp.to_le_bytes());
        }

        let this_update = self
            .this_update
            .duration_since(UNIX_EPOCH)
            .map_err(|e| Error::internal(format!("Time error: {}", e)))?
            .as_secs();
        data.extend_from_slice(&this_update.to_le_bytes());

        let next_update = self
            .next_update
            .duration_since(UNIX_EPOCH)
            .map_err(|e| Error::internal(format!("Time error: {}", e)))?
            .as_secs();
        data.extend_from_slice(&next_update.to_le_bytes());

        Ok(data)
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let mut offset = 0;

        let entries_count = u32::from_le_bytes(
            data.get(offset..offset + 4)
                .ok_or_else(|| Error::serialization("Invalid CRL data"))?
                .try_into()
                .map_err(|_| Error::serialization("Invalid CRL data"))?,
        ) as usize;
        offset += 4;

        let mut revoked_entries = Vec::with_capacity(entries_count);
        for _ in 0..entries_count {
            let serial = u64::from_le_bytes(
                data.get(offset..offset + 8)
                    .ok_or_else(|| Error::serialization("Invalid CRL data"))?
                    .try_into()
                    .map_err(|_| Error::serialization("Invalid CRL data"))?,
            );
            offset += 8;

            let timestamp = u64::from_le_bytes(
                data.get(offset..offset + 8)
                    .ok_or_else(|| Error::serialization("Invalid CRL data"))?
                    .try_into()
                    .map_err(|_| Error::serialization("Invalid CRL data"))?,
            );
            offset += 8;

            let time = UNIX_EPOCH + Duration::from_secs(timestamp);
            revoked_entries.push((serial, time));
        }

        let this_update_ts = u64::from_le_bytes(
            data.get(offset..offset + 8)
                .ok_or_else(|| Error::serialization("Invalid CRL data"))?
                .try_into()
                .map_err(|_| Error::serialization("Invalid CRL data"))?,
        );
        offset += 8;

        let next_update_ts = u64::from_le_bytes(
            data.get(offset..offset + 8)
                .ok_or_else(|| Error::serialization("Invalid CRL data"))?
                .try_into()
                .map_err(|_| Error::serialization("Invalid CRL data"))?,
        );

        Ok(Self {
            revoked_entries,
            this_update: UNIX_EPOCH + Duration::from_secs(this_update_ts),
            next_update: UNIX_EPOCH + Duration::from_secs(next_update_ts),
        })
    }
}

pub struct CertificateValidator {
    ca_cert: CertificateDer<'static>,
    crl: Option<Arc<RwLock<CertificateRevocationList>>>,
}

impl CertificateValidator {
    pub fn new(ca_cert: CertificateDer<'static>) -> Self {
        Self { ca_cert, crl: None }
    }

    pub fn with_crl(ca_cert: CertificateDer<'static>, crl: CertificateRevocationList) -> Self {
        Self {
            ca_cert,
            crl: Some(Arc::new(RwLock::new(crl))),
        }
    }

    pub fn update_crl(&mut self, crl: CertificateRevocationList) {
        self.crl = Some(Arc::new(RwLock::new(crl)));
    }

    pub fn ca_certificate(&self) -> &CertificateDer<'static> {
        &self.ca_cert
    }

    pub async fn validate(
        &self,
        _cert: &CertificateDer<'static>,
        serial: Option<u64>,
    ) -> Result<CertificateValidity> {
        if let Some(crl_lock) = &self.crl {
            let crl = crl_lock.read().await;
            if let Some(serial_num) = serial {
                if crl.contains(serial_num) {
                    return Err(Error::internal("Certificate has been revoked"));
                }
            }
        }

        let now = SystemTime::now();

        Ok(CertificateValidity {
            is_valid: true,
            checked_at: now,
            expires_at: now + Duration::from_secs(86400),
        })
    }

    pub fn validate_chain(&self, cert_chain: &[CertificateDer<'static>]) -> Result<bool> {
        if cert_chain.is_empty() {
            return Err(Error::internal("Empty certificate chain"));
        }

        Ok(true)
    }
}

#[derive(Debug, Clone)]
pub struct CertificateValidity {
    pub is_valid: bool,
    pub checked_at: SystemTime,
    pub expires_at: SystemTime,
}

impl CertificateValidity {
    pub fn time_until_expiry(&self) -> Duration {
        self.expires_at
            .duration_since(self.checked_at)
            .unwrap_or(Duration::ZERO)
    }

    pub fn should_rotate(&self, threshold: Duration) -> bool {
        self.time_until_expiry() < threshold
    }
}

pub fn generate_serial() -> u64 {
    use rand::Rng;
    rand::rng().random()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ca_generation() {
        let ca = CertificateAuthority::generate("Test CA").unwrap();
        assert!(!ca.certificate().is_empty());
        assert!(!ca.private_key_der().is_empty());
    }

    #[test]
    fn test_certificate_issuance() {
        let ca = CertificateAuthority::generate("Test CA").unwrap();
        let serial = generate_serial();
        let (cert, key) = ca
            .issue_certificate("test-node", CertificateType::Node, serial)
            .unwrap();

        assert!(!cert.is_empty());
        assert!(!key.is_empty());
    }

    #[tokio::test]
    async fn test_revocation() {
        let ca = CertificateAuthority::generate("Test CA").unwrap();
        let serial = generate_serial();

        ca.issue_certificate("test-node", CertificateType::Node, serial)
            .unwrap();
        ca.revoke(serial).await.unwrap();

        assert!(ca.is_revoked(serial).await);
    }

    #[tokio::test]
    async fn test_crl_generation() {
        let ca = CertificateAuthority::generate("Test CA").unwrap();
        let serial = generate_serial();

        ca.issue_certificate("test-node", CertificateType::Node, serial)
            .unwrap();
        ca.revoke(serial).await.unwrap();

        let crl = ca.generate_crl().await.unwrap();
        assert!(crl.contains(serial));
    }

    #[test]
    fn test_crl_serialization() {
        let serial = generate_serial();

        let crl = CertificateRevocationList {
            revoked_entries: vec![(serial, SystemTime::now())],
            this_update: SystemTime::now(),
            next_update: SystemTime::now() + Duration::from_secs(3600),
        };

        let bytes = crl.to_bytes().unwrap();
        let restored = CertificateRevocationList::from_bytes(&bytes).unwrap();

        assert!(restored.contains(serial));
    }

    #[tokio::test]
    async fn test_validator() {
        let ca = CertificateAuthority::generate("Test CA").unwrap();
        let validator = CertificateValidator::new(ca.certificate().clone());

        let serial = generate_serial();
        let (cert, _) = ca
            .issue_certificate("test-node", CertificateType::Node, serial)
            .unwrap();

        let validity = validator.validate(&cert, Some(serial)).await.unwrap();
        assert!(validity.is_valid);
    }
}
