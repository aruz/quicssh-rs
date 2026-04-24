use clap::Parser;
use quinn::{crypto, Endpoint, ServerConfig, VarInt};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};

use log::{debug, error, info};
use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::{net::SocketAddr, sync::Arc};
use tokio::fs::read_to_string;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[derive(Parser, Debug)]
#[clap(name = "server")]
pub struct Opt {
    /// Address to listen on
    #[clap(long = "listen", short = 'l', default_value = "0.0.0.0:4433")]
    listen: SocketAddr,
    /// Address of the ssh server
    #[clap(long = "proxy-to", short = 'p')]
    proxy_to: Option<SocketAddr>,
    #[clap(long = "conf", short = 'F')]
    conf_path: Option<PathBuf>,
    /// MTU upper bound: numeric value (e.g., 1200) or "safety" for RFC-compliant 1200 bytes
    #[clap(long = "mtu-upper-bound")]
    mtu_upper_bound: Option<String>,
}

/// Returns default server configuration along with its certificate.
///
/// # Arguments
/// * `mtu_upper_bound` - Optional MTU upper bound in bytes. None uses Quinn's default (1452).
#[cfg_attr(not(any(windows, target_os = "linux")), allow(unused_variables))]
fn configure_server(
    mtu_upper_bound: Option<u16>,
) -> Result<(ServerConfig, Vec<u8>), Box<dyn Error>> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let cert_der = cert.cert.der().to_vec();
    let priv_key = PrivateKeyDer::try_from(cert.signing_key.serialize_der())
        .map_err(|e| -> Box<dyn Error> { e.into() })?;
    let cert_chain = vec![CertificateDer::from(cert_der.clone())];

    let mut server_config = ServerConfig::with_single_cert(cert_chain, priv_key)?;
    let transport_config = Arc::get_mut(&mut server_config.transport).unwrap();
    transport_config.max_concurrent_uni_streams(0_u8.into());
    transport_config.max_idle_timeout(Some(VarInt::from_u32(60_000).into()));
    transport_config.keep_alive_interval(Some(std::time::Duration::from_secs(1)));
    #[cfg(any(windows, target_os = "linux"))]
    {
        if let Some(mtu) = mtu_upper_bound {
            // Set MTU discovery upper bound per RFC 9000 Section 14.1
            // and RFC 8899 Section 5.1.2 (recommended BASE_PLPMTU for UDP).
            // 1200 bytes ensures compatibility with IPv6 minimum MTU (1280 per RFC 8200).
            let mut mtu_config = quinn::MtuDiscoveryConfig::default();
            mtu_config.upper_bound(mtu);
            transport_config.mtu_discovery_config(Some(mtu_config));
        }
        // If mtu_upper_bound is None, use Quinn's default MTU discovery (1452 bytes)
    }

    Ok((server_config, cert_der))
}

#[allow(unused)]
fn make_server_endpoint(
    bind_addr: SocketAddr,
    mtu_upper_bound: Option<u16>,
) -> Result<(Endpoint, Vec<u8>), Box<dyn Error>> {
    let (server_config, server_cert) = configure_server(mtu_upper_bound)?;
    let endpoint = Endpoint::server(server_config, bind_addr)?;
    Ok((endpoint, server_cert))
}

#[derive(Deserialize, Debug)]
struct ServerConf {
    proxy: HashMap<String, SocketAddr>,
}
impl ServerConf {
    fn new() -> Self {
        ServerConf {
            proxy: HashMap::<String, SocketAddr>::new(),
        }
    }
}

/// Determines the default proxy address based on configuration and options
///
/// Priority order:
/// 1. "default" key in TOML conf
/// 2. --proxy-to option
/// 3. localhost:22 (fallback)
fn determine_default_proxy(conf: &ServerConf, proxy_to_option: Option<SocketAddr>) -> SocketAddr {
    match conf.proxy.get("default") {
        Some(sock) => *sock,
        None => proxy_to_option.unwrap_or(SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 22)),
    }
}

#[tokio::main]
pub async fn run(options: Opt) -> Result<(), Box<dyn Error>> {
    let conf: ServerConf = match options.conf_path {
        Some(path) => {
            info!("[server] importing conf file: {}", path.display());
            toml::from_str(&(read_to_string(path).await?))?
        }
        None => ServerConf::new(),
    };

    let default_proxy = determine_default_proxy(&conf, options.proxy_to);
    info!("[server] default proxy aim: {}", default_proxy);

    // Parse MTU upper bound option
    let mtu_upper_bound = match &options.mtu_upper_bound {
        Some(s) if s == "safety" => Some(1200),
        Some(s) => Some(s.parse::<u16>().map_err(|_| "Invalid MTU value")?),
        None => None,
    };

    let (endpoint, _) = make_server_endpoint(options.listen, mtu_upper_bound).unwrap();
    info!("[server] listening on: {}", options.listen);
    // accept a single connection
    loop {
        let incoming_conn = match endpoint.accept().await {
            Some(conn) => conn,
            None => {
                continue;
            }
        };
        let conn = match incoming_conn.await {
            Ok(conn) => conn,
            Err(e) => {
                error!("[server] accept connection error: {}", e);
                continue;
            }
        };

        let sni = conn
            .handshake_data()
            .unwrap()
            .downcast::<crypto::rustls::HandshakeData>()
            .unwrap()
            .server_name
            .unwrap_or(conn.remote_address().ip().to_string());
        let proxy_to = *conf.proxy.get(&sni).unwrap_or(&default_proxy);
        info!(
            "[server] connection accepted: ({}, {}) -> {}",
            conn.remote_address(),
            sni,
            proxy_to
        );
        tokio::spawn(async move {
            handle_connection(proxy_to, conn).await;
        });
        // Dropping all handles associated with a connection implicitly closes it
    }
}

async fn handle_connection(proxy_for: SocketAddr, connection: quinn::Connection) {
    let ssh_stream = TcpStream::connect(proxy_for).await;
    let ssh_conn = match ssh_stream {
        Ok(conn) => conn,
        Err(e) => {
            error!("[server] connect to ssh error: {}", e);
            return;
        }
    };

    info!("[server] ssh connection established");

    let (mut quinn_send, mut quinn_recv) = match connection.accept_bi().await {
        Ok(stream) => stream,
        Err(e) => {
            error!("[server] open quic stream error: {}", e);
            return;
        }
    };

    let (mut ssh_recv, mut ssh_write) = tokio::io::split(ssh_conn);

    let recv_thread = async move {
        let mut buf = [0; 2048];
        loop {
            match ssh_recv.read(&mut buf).await {
                Ok(n) => {
                    // Upstream ssh half-closed: finish the quic send stream and exit.
                    if n == 0 {
                        debug!("[server] ssh EOF, finishing quic send stream");
                        if let Err(e) = quinn_send.finish() {
                            debug!("[server] quinn_send.finish on exit: {}", e);
                        }
                        return;
                    }
                    debug!("[server] recv data from ssh server {} bytes", n);
                    match quinn_send.write_all(&buf[..n]).await {
                        Ok(_) => (),
                        Err(e) => {
                            error!("[server] writing to quic stream error: {}", e);
                            return;
                        }
                    }
                }
                Err(e) => {
                    error!("[server] reading from ssh server error: {}", e);
                    return;
                }
            }
        }
    };

    let write_thread = async move {
        let mut buf = [0; 2048];
        loop {
            match quinn_recv.read(&mut buf).await {
                // Peer finished its send stream: shut down the ssh write half and exit.
                Ok(None) => {
                    debug!("[server] quic stream finished by peer");
                    if let Err(e) = ssh_write.shutdown().await {
                        debug!("[server] ssh_write.shutdown on exit: {}", e);
                    }
                    return;
                }
                Ok(Some(n)) => {
                    debug!("[server] recv data from quic stream {} bytes", n);
                    match ssh_write.write_all(&buf[..n]).await {
                        Ok(_) => (),
                        Err(e) => {
                            error!("[server] writing to ssh server error: {}", e);
                            return;
                        }
                    }
                }
                Err(e) => {
                    error!("[server] reading from quic client error: {}", e);
                    return;
                }
            }
        }
    };

    tokio::select! {
        _ = recv_thread => (),
        _ = write_thread => (),
    }

    info!("[server] exit client");

    // tokio::join!(recv_thread, write_thread);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_configure_server_with_mtu() {
        let result = configure_server(Some(1200));
        assert!(result.is_ok());
        let (_server_config, cert) = result.unwrap();

        // Verify that a certificate was generated (DER format is non-empty)
        assert!(!cert.is_empty());
        // Typical self-signed cert DER is several hundred bytes
        assert!(cert.len() > 100);
    }

    #[test]
    fn test_configure_server_without_mtu() {
        let result = configure_server(None);
        assert!(result.is_ok());
        let (_server_config, cert) = result.unwrap();

        // Verify that a certificate was generated (DER format is non-empty)
        assert!(!cert.is_empty());
        // Typical self-signed cert DER is several hundred bytes
        assert!(cert.len() > 100);
    }

    #[test]
    fn test_configure_server_creates_valid_config() {
        // Test with MTU
        let result1 = configure_server(Some(1200));
        assert!(result1.is_ok());

        // Test without MTU
        let result2 = configure_server(None);
        assert!(result2.is_ok());

        // Both configs should produce different certificates (different keys)
        let (_, cert1) = result1.unwrap();
        let (_, cert2) = result2.unwrap();
        // Different invocations generate different certs
        assert_ne!(cert1, cert2);
    }

    #[test]
    fn test_determine_default_proxy_from_conf() {
        let mut conf = ServerConf::new();
        let default_addr: SocketAddr = "192.168.1.1:2222".parse().unwrap();
        conf.proxy.insert("default".to_string(), default_addr);

        let result = determine_default_proxy(&conf, Some("10.0.0.1:3333".parse().unwrap()));
        assert_eq!(result, default_addr); // Conf takes priority
    }

    #[test]
    fn test_determine_default_proxy_from_option() {
        let conf = ServerConf::new();
        let option_addr: SocketAddr = "10.0.0.1:3333".parse().unwrap();

        let result = determine_default_proxy(&conf, Some(option_addr));
        assert_eq!(result, option_addr);
    }

    #[test]
    fn test_determine_default_proxy_fallback() {
        let conf = ServerConf::new();

        let result = determine_default_proxy(&conf, None);
        assert_eq!(result, SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 22));
    }

    #[test]
    fn test_server_conf_new() {
        let conf = ServerConf::new();
        assert!(conf.proxy.is_empty());
    }
}
