use std::{env, path::PathBuf};

use opcua::client::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = env::var("OPC_URL").ok().unwrap_or("opc.tcp://localhost:4840/".into());
    log::info!("Reaching {url}");

    fn make_certificate_store() -> CertificateStore {
        let cert_store = CertificateStore::new(&PathBuf::from("./pki"));
        assert!(cert_store.ensure_pki_path().is_ok());
        cert_store
    }

    let cert_store = make_certificate_store();
    let _ = cert_store.create_and_store_application_instance_cert(
        &X509Data {
            key_size: 2048,
            common_name: "x".to_string(),
            organization: "x.org".to_string(),
            organizational_unit: "x.org ops".to_string(),
            country: "EN".to_string(),
            state: "London".to_string(),
            alt_host_names: vec!["host1".to_string(), "host2".to_string()],
            certificate_duration_days: 60,
        },
        false,
    )?;

    // Optional - enable OPC UA logging
    opcua::console_logging::init();

    // The client API has a simple `find_servers` function that connects and returns servers for us.
    let mut client = Client::new(ClientConfig::new("DiscoveryClient", "urn:DiscoveryClient"));

    let servers = client.find_servers(url)?;
    log::info!("Discovered {} servers", servers.len());

    for srv in servers {
        log::info!("Found {:?} ({:?}): {srv:?}", srv.application_name, srv.application_type);

        for disco_url in srv.discovery_urls.unwrap_or_default() {
            log::info!("> Disco URL {disco_url}");

            if !is_opc_ua_binary_url(disco_url.as_ref()) {
                continue;
            }

            let clt = Client::new(ClientConfig::new("discovery-client", "urn:discovery-client"));

            let endpoints = clt.get_server_endpoints_from_url(disco_url)?;
            for endpt in endpoints {
                log::info!(">> Endpoint {endpt:?}");
            }
        }
    }

    Ok(())
}
