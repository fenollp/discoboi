use std::{env, str::FromStr};

use env_logger::Env;
use opcua::{
    client::prelude::{Client, ClientConfig},
    core::comms::url::is_opc_ua_binary_url,
    crypto::SecurityPolicy,
    types::{ApplicationDescription, EndpointDescription},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();

    let url = env::var("OPC_URL").ok().unwrap_or("opc.tcp://localhost:4840/".to_owned());
    log::info!("Reaching {url}");

    let mut client = Client::new(ClientConfig {
        application_name: "UaBrowser@mbp".to_owned(),
        application_uri: "urn:mbp:ProsysOPC:UaBrowser".to_owned(),
        ..Default::default()
    });
    let servers = client.find_servers(url)?;
    log::info!("Discovered {} servers", servers.len());

    for ApplicationDescription { application_name, application_type, discovery_urls, .. } in servers
    {
        log::info!("Server {application_name} ({application_type:?})");

        for disco_url in discovery_urls.unwrap_or_default() {
            log::info!("> Disco URL {disco_url}");

            if !is_opc_ua_binary_url(disco_url.as_ref()) {
                log::warn!(">> Skipping: !is_opc_ua_binary_url");
                continue;
            }

            let clt = Client::new(ClientConfig::new("discovery-client", "urn:discovery-client"));
            let endpoints = clt.get_server_endpoints_from_url(disco_url)?;
            for endpoint in endpoints {
                let EndpointDescription {
                    endpoint_url, security_mode, security_policy_uri, ..
                } = endpoint;

                let secpol = SecurityPolicy::from_str(security_policy_uri.as_ref()).unwrap();

                log::info!(">> Endpoint {endpoint_url} SecPol:{secpol} SecMode:{security_mode}");
            }
        }
    }

    Ok(())
}
