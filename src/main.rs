use std::{
    env,
    str::FromStr,
    sync::{Arc, RwLock},
};

use env_logger::Env;
use log::{debug, info, warn};
use opcua::{
    client::prelude::{
        Client, ClientConfig, DataChangeCallback, IdentityToken, MonitoredItemService, Session,
        SubscriptionService,
    },
    core::comms::url::is_opc_ua_binary_url,
    crypto::SecurityPolicy,
    types::{
        ApplicationDescription, DataValue, EndpointDescription, MessageSecurityMode, NodeId,
        TimestampsToReturn, UserTokenPolicy,
    },
};

type Errr = Box<dyn std::error::Error>;

fn main() -> Result<(), Errr> {
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();

    let url = env::var("OPC_URL").ok().unwrap_or("opc.tcp://localhost:4840/".to_owned());
    info!("Reaching {url}");

    let mut client = Client::new(ClientConfig {
        application_name: "UaBrowser@mbp".to_owned(),
        application_uri: "urn:mbp:ProsysOPC:UaBrowser".to_owned(),
        ..Default::default()
    });
    let servers = client.find_servers(url)?;
    info!("Discovered {} servers", servers.len());

    for ApplicationDescription { application_name, application_type, discovery_urls, .. } in servers
    {
        info!("Server {application_name} ({application_type:?})");

        for disco_url in discovery_urls.unwrap_or_default() {
            info!("> Disco URL {disco_url}");

            if !is_opc_ua_binary_url(disco_url.as_ref()) {
                warn!(">> Skipping: !is_opc_ua_binary_url");
                continue;
            }

            let mut clt =
                Client::new(ClientConfig::new("discovery-client", "urn:discovery-client"));
            for endpoint in clt.get_server_endpoints_from_url(disco_url)? {
                let EndpointDescription {
                    endpoint_url, security_mode, security_policy_uri, ..
                } = endpoint;

                let secpol = SecurityPolicy::from_str(security_policy_uri.as_ref()).unwrap();
                info!(">> Endpoint {endpoint_url} {security_mode}: {secpol}");

                if security_mode != MessageSecurityMode::None {
                    continue;
                }
                if secpol != SecurityPolicy::None {
                    continue;
                }

                debug!(">>> Connecting...");
                let session = clt.connect_to_endpoint(
                    (
                        endpoint_url.as_ref(),
                        SecurityPolicy::None.to_str(),
                        MessageSecurityMode::None,
                        UserTokenPolicy::anonymous(),
                    ),
                    IdentityToken::Anonymous,
                )?;

                debug!(">>> Subscribing...");
                sub(session.clone());

                warn!(">>> Listening...");
                // Synchronously runs a polling loop over the supplied session.
                Session::run(session)
            }
        }
    }

    Ok(())
}

fn sub(session: Arc<RwLock<Session>>) -> Result<(), Errr> {
    let session = session.read()?;

    let subscription_id = session.create_subscription(
        2000.0, // publishing_interval
        10,     // lifetime_count
        30,     // max_keep_alive_count
        0,      // max_notifications_per_publish
        0,      // priority
        true,   // publishing_enabled
        DataChangeCallback::new(|changed_monitored_items| {
            info!(">>> Got a change notif! {changed_monitored_items:?}");
            for &item in changed_monitored_items {
                let node_id = &item.item_to_monitor().node_id;
                let DataValue { value, status, .. } = item.last_value();
                info!(">>>> Item {node_id} value:{value:?} err:{status:?}");
            }
        }),
    )?;
    debug!(">>> Created subscription {subscription_id}");

    info!(">>> Using subscription to monitor root node");
    let _ = session.create_monitored_items(
        subscription_id,
        TimestampsToReturn::Both,
        &[
            NodeId::root_folder_id().into(),
            NodeId::objects_folder_id().into(),
            NodeId::types_folder_id().into(),
            NodeId::views_folder_id().into(),
        ],
    )?;

    Ok(())
}
