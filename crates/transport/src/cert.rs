//! Ephemeral TLS material plus the "trust nothing at TLS" verifier
//! that delegates real authentication to the application layer.
//!
//! The TLS handshake here exists *only* for confidentiality and
//! channel-binding properties. Whether we should actually talk to the
//! peer is decided after the handshake by checking a signed `Hello`
//! frame against the [`PeerStore`](crate::peer_store::PeerStore).

use crate::{Error, Result};
use rcgen::{CertificateParams, DistinguishedName};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use std::sync::Arc;

/// Generate an ephemeral self-signed cert + key suitable for TLS.
pub fn ephemeral_self_signed() -> Result<(CertificateDer<'static>, PrivatePkcs8KeyDer<'static>)> {
    let mut params = CertificateParams::new(vec!["borderless".into()])?;
    params.distinguished_name = DistinguishedName::new();
    let key = rcgen::KeyPair::generate()?;
    let cert = params.self_signed(&key)?;
    let cert_der = CertificateDer::from(cert);
    let key_der = PrivatePkcs8KeyDer::from(key.serialize_der());
    Ok((cert_der, key_der))
}

/// `ServerCertVerifier` that accepts any well-formed peer certificate.
///
/// This is intentional. Real identity is checked at the application
/// layer via a signed `Hello` bound to the TLS exporter (see
/// [`Identity::sign`](crate::identity::Identity::sign)).
#[derive(Debug)]
pub struct AcceptAnyServerCert;

impl ServerCertVerifier for AcceptAnyServerCert {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::ED25519,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
        ]
    }
}

/// Ensure the rustls CryptoProvider is installed; safe to call many times.
pub fn ensure_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// Build a rustls `ClientConfig` that delegates auth to the app layer.
pub fn client_config() -> Result<Arc<rustls::ClientConfig>> {
    ensure_crypto_provider();
    let cfg = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAnyServerCert))
        .with_no_client_auth();
    Ok(Arc::new(cfg))
}

/// Build a rustls `ServerConfig` from `(cert, key)`.
pub fn server_config(
    cert: CertificateDer<'static>,
    key: PrivatePkcs8KeyDer<'static>,
) -> Result<Arc<rustls::ServerConfig>> {
    ensure_crypto_provider();
    let cfg = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key.into())
        .map_err(Error::Rustls)?;
    Ok(Arc::new(cfg))
}
