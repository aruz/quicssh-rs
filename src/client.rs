// #![cfg(feature = "rustls")]

use clap::Parser;
use quinn::{ClientConfig, Endpoint, VarInt};
use std::{error::Error, net::SocketAddr, sync::Arc};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[cfg(not(windows))]
use tokio::signal::unix::{signal, SignalKind};
#[cfg(windows)]
use tokio::signal::windows::ctrl_c;
use url::Url;

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn, Level};

#[derive(Parser, Debug)]
#[clap(name = "client")]
pub struct Opt {
    /// Server address
    url: Url,
    /// Client address
    #[clap(long = "bind", short = 'b')]
    bind_addr: Option<SocketAddr>,
    /// MTU upper bound: numeric value (e.g., 1200) or "safety" for RFC-compliant 1200 bytes
    #[clap(long = "mtu-upper-bound")]
    mtu_upper_bound: Option<String>,
}

/// Enables MTUD if supported by the operating system
///
/// # Arguments
/// * `upper_bound` - Optional MTU upper bound in bytes. None uses Quinn's default (1452).
#[cfg(not(any(windows, target_os = "linux")))]
fn enable_mtud_if_supported(_upper_bound: Option<u16>) -> quinn::TransportConfig {
    quinn::TransportConfig::default()
}

/// Enables MTUD if supported by the operating system
///
/// # Arguments
/// * `upper_bound` - Optional MTU upper bound in bytes. None uses Quinn's default (1452).
#[cfg(any(windows, target_os = "linux"))]
fn enable_mtud_if_supported(upper_bound: Option<u16>) -> quinn::TransportConfig {
    let mut transport_config = quinn::TransportConfig::default();

    if let Some(mtu) = upper_bound {
        // Set MTU discovery upper bound per RFC 9000 Section 14.1
        // and RFC 8899 Section 5.1.2 (recommended BASE_PLPMTU for UDP).
        // 1200 bytes ensures compatibility with IPv6 minimum MTU (1280 per RFC 8200).
        let mut mtu_config = quinn::MtuDiscoveryConfig::default();
        mtu_config.upper_bound(mtu);
        transport_config.mtu_discovery_config(Some(mtu_config));
    }
    // If upper_bound is None, use Quinn's default MTU discovery (1452 bytes)

    transport_config
}

struct SkipServerVerification;

impl SkipServerVerification {
    fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl rustls::client::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::Certificate,
        _intermediates: &[rustls::Certificate],
        _server_name: &rustls::ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: std::time::SystemTime,
    ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::ServerCertVerified::assertion())
    }
}

fn configure_client(mtu_upper_bound: Option<u16>) -> Result<ClientConfig, Box<dyn Error>> {
    let crypto = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_custom_certificate_verifier(SkipServerVerification::new())
        .with_no_client_auth();

    let mut client_config = ClientConfig::new(Arc::new(crypto));
    let mut transport_config = enable_mtud_if_supported(mtu_upper_bound);
    transport_config.max_idle_timeout(Some(VarInt::from_u32(60_000).into()));
    transport_config.keep_alive_interval(Some(std::time::Duration::from_secs(1)));
    client_config.transport_config(Arc::new(transport_config));

    Ok(client_config)
}

/// Validates that the URL scheme is "quic"
///
/// # Arguments
/// * `url` - URL to validate
///
/// # Returns
/// * `Ok(())` if the scheme is "quic"
/// * `Err` if the scheme is not "quic"
fn validate_url_scheme(url: &Url) -> Result<(), Box<dyn Error>> {
    if url.scheme() != "quic" {
        return Err("URL scheme must be quic".into());
    }
    Ok(())
}

/// Constructs a QUIC endpoint configured for use a client only.
///
/// ## Args
///
/// - bind_addr: local address to bind to
/// - mtu_upper_bound: optional MTU upper bound in bytes
#[allow(unused)]
fn make_client_endpoint(
    bind_addr: SocketAddr,
    mtu_upper_bound: Option<u16>,
) -> Result<Endpoint, Box<dyn Error>> {
    let client_cfg = configure_client(mtu_upper_bound)?;
    let mut endpoint = Endpoint::client(bind_addr)?;
    endpoint.set_default_client_config(client_cfg);
    Ok(endpoint)
}

#[tokio::main]
pub async fn run(options: Opt) -> Result<(), Box<dyn Error>> {
    let url = options.url;
    validate_url_scheme(&url)?;

    // Parse MTU upper bound option
    let mtu_upper_bound = match &options.mtu_upper_bound {
        Some(s) if s == "safety" => Some(1200),
        Some(s) => Some(s.parse::<u16>().map_err(|_| "Invalid MTU value")?),
        None => None,
    };

    // Currently `url` crate doesn't recognize quic as scheme (see socket_addrs()), so we can set default port using argument. In future if quic default port is added (as 80 or 443, likely), we will fail to connect to proper port. Ideally we should define own scheme. (ex. "qsrs://" abbr of quicssh-rs)
    let sock_list = url
        .socket_addrs(|| Some(4433))
        .map_err(|_| "Couldn't resolve to any address")?;

    // Currently we only use the first addr. The other addrs should be fallbacks of the connection, but not implemented now.
    let remote = sock_list[0];
    let sni = url.host_str().unwrap_or("THIS_HOSTNAME_SHOULD_NOT_BE_USED");

    // Remove brackets from IPv6 address
    let sni = sni.trim_start_matches('[').trim_end_matches(']');

    info!("[client] Connecting to: {} <- {}", remote, sni);

    let endpoint = make_client_endpoint(
        match options.bind_addr {
            Some(local) => local,
            None => {
                use std::net::{IpAddr::*, Ipv4Addr, Ipv6Addr};
                if remote.is_ipv6() {
                    SocketAddr::new(V6(Ipv6Addr::UNSPECIFIED), 0)
                } else {
                    SocketAddr::new(V4(Ipv4Addr::UNSPECIFIED), 0)
                }
            }
        },
        mtu_upper_bound,
    )?;
    // connect to server
    let connection = endpoint.connect(remote, sni).unwrap().await.unwrap();
    info!(
        "[client] Connected to: {} <- {}",
        connection.remote_address(),
        sni
    );

    let (mut send, mut recv) = connection
        .open_bi()
        .await
        .map_err(|e| format!("failed to open stream: {}", e))?;

    let recv_thread = async move {
        let mut buf = vec![0; 2048];
        let mut writer = tokio::io::BufWriter::new(tokio::io::stdout());

        loop {
            match recv.read(&mut buf).await {
                // Return value of `Ok(0)` signifies that the remote has
                // closed
                Ok(None) => {
                    continue;
                }
                Ok(Some(n)) => {
                    debug!("[client] recv data from quic server {} bytes", n);
                    // Copy the data back to socket
                    match writer.write_all(&buf[..n]).await {
                        Ok(_) => (),
                        Err(e) => {
                            error!("[client] write to stdout error: {}", e);
                            return;
                        }
                    }
                }
                Err(err) => {
                    // Unexpected socket error. There isn't much we can do
                    // here so just stop processing.
                    error!("[client] recv data from quic server error: {}", err);
                    return;
                }
            }
            if writer.flush().await.is_err() {
                error!("[client] recv data flush stdout error");
            }
        }
    };

    let write_thread = async move {
        let mut buf = [0; 2048];
        let mut reader = tokio::io::BufReader::new(tokio::io::stdin());

        loop {
            match reader.read(&mut buf).await {
                // Return value of `Ok(0)` signifies that the remote has
                // closed
                Ok(n) => {
                    if n == 0 {
                        continue;
                    }
                    debug!("[client] recv data from stdin {} bytes", n);
                    // Copy the data back to socket
                    if send.write_all(&buf[..n]).await.is_err() {
                        // Unexpected socket error. There isn't much we can
                        // do here so just stop processing.
                        info!("[client] send data to quic server error");
                        return;
                    }
                }
                Err(err) => {
                    // Unexpected socket error. There isn't much we can do
                    // here so just stop processing.
                    info!("[client] recv data from stdin error: {}", err);
                    return;
                }
            }
        }
    };

    let signal_thread = create_signal_thread();

    tokio::select! {
        _ = recv_thread => (),
        _ = write_thread => (),
        _ = signal_thread => connection.close(0u32.into(), b"signal HUP"),
    }

    info!("[client] exit client");

    Ok(())
}

#[cfg(windows)]
async fn create_signal_thread() {
    let mut stream = match ctrl_c() {
        Ok(s) => s,
        Err(e) => {
            error!("[client] create signal stream error: {}", e);
            return;
        }
    };

    stream.recv().await;
    info!("[client] got signal Ctrl-C");
}
#[cfg(not(windows))]
async fn create_signal_thread() {
    let mut stream = match signal(SignalKind::hangup()) {
        Ok(s) => s,
        Err(e) => {
            error!("[client] create signal stream error: {}", e);
            return;
        }
    };

    stream.recv().await;
    info!("[client] got signal HUP");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_url_scheme_valid() {
        let url = Url::parse("quic://example.com:4433").unwrap();
        assert!(validate_url_scheme(&url).is_ok());
    }

    #[test]
    fn test_validate_url_scheme_invalid() {
        let url = Url::parse("http://example.com:4433").unwrap();
        assert!(validate_url_scheme(&url).is_err());

        let url = Url::parse("https://example.com:4433").unwrap();
        assert!(validate_url_scheme(&url).is_err());
    }

    #[test]
    #[cfg(any(windows, target_os = "linux"))]
    fn test_enable_mtud_if_supported_with_custom_mtu() {
        // Just verify that the function completes successfully with Some(1200)
        let _transport_config = enable_mtud_if_supported(Some(1200));
        // MTU config is applied internally; we can't directly inspect it
        // but we verify no panic occurs
    }

    #[test]
    #[cfg(any(windows, target_os = "linux"))]
    fn test_enable_mtud_if_supported_with_none() {
        // Verify that the function completes successfully with None
        let _transport_config = enable_mtud_if_supported(None);
        // Default Quinn behavior is used; verify no panic occurs
    }

    #[test]
    #[cfg(not(any(windows, target_os = "linux")))]
    fn test_enable_mtud_if_supported_unsupported_platform() {
        // On unsupported platforms, the function should return a default config
        let _transport_config = enable_mtud_if_supported(Some(1200));
        // Just verify it doesn't panic
    }
}
