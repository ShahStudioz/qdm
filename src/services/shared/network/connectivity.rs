//! # Network Connectivity Monitor Module
//!
//! Provides fast, lightweight asynchronous network reachability checks to distinguish
//! between transient network outages (e.g. Wi-Fi disconnection / offline) and remote server errors.

use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpStream;

pub struct ConnectivityMonitor;

impl ConnectivityMonitor {
    /// Asynchronously checks if the system currently has active internet connectivity.
    ///
    /// Probes fast public DNS TCP endpoints (`1.1.1.1:53`, `8.8.8.8:53`) with a 1.2s timeout,
    /// falling back to standard HTTP 204 endpoints.
    pub async fn is_online() -> bool {
        // Fast probe: TCP ping to Cloudflare / Google DNS
        let targets = [
            "1.1.1.1:53".parse::<SocketAddr>().unwrap(),
            "8.8.8.8:53".parse::<SocketAddr>().unwrap(),
            "1.0.0.1:53".parse::<SocketAddr>().unwrap(),
        ];

        for addr in targets {
            if tokio::time::timeout(Duration::from_millis(1200), TcpStream::connect(addr))
                .await
                .is_ok_and(|res| res.is_ok())
            {
                return true;
            }
        }

        // Secondary fallback: Fast HTTP connectivity probe
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(1500))
            .build();

        if let Ok(client) = client {
            if let Ok(resp) = client
                .get("http://cp.cloudflare.com/generate_204")
                .send()
                .await
            {
                if resp.status().is_success() || resp.status().as_u16() == 204 {
                    return true;
                }
            }
        }

        false
    }
}
