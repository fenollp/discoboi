use std::{env, str::FromStr, sync::Arc};

use env_logger::Env;
use log::{debug, info, warn};
use opcua::{
    client::prelude::{Client, ClientConfig, IdentityToken, Session, ViewService},
    core::comms::url::is_opc_ua_binary_url,
    crypto::SecurityPolicy,
    sync::RwLock,
    types::{
        ApplicationDescription, BrowseDescription, BrowseDescriptionResultMask, BrowseDirection,
        BrowseResult, EndpointDescription, MessageSecurityMode, NodeClassMask, NodeId,
        ReferenceTypeId, UserTokenPolicy,
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

                debug!(">>> Browsing...");
                browse(session, NodeId::root_folder_id())?;
            }
        }
    }

    Ok(())
}

fn browse(session: Arc<RwLock<Session>>, node_id: NodeId) -> Result<(), Errr> {
    let rlock = session.read();
    let Some(res) = rlock.browse(&[BrowseDescription {
        node_id,
        browse_direction: BrowseDirection::Both,
        reference_type_id: ReferenceTypeId::HierarchicalReferences.into(),
        include_subtypes: true,
        node_class_mask: NodeClassMask::all().bits(),
        result_mask: BrowseDescriptionResultMask::all().bits(),
    }])?
    else {
        return Ok(());
    };

    do_browse(session.clone(), &res)
}

#[test]
fn wth_typing_exists_for_a_reason() {
    use opcua::types::{BrowseResultMask, NodeClass};

    assert_eq!(63, BrowseDescriptionResultMask::all().bits());
    assert_eq!(63, BrowseResultMask::All as u32);
    assert_eq!(255, NodeClassMask::all().bits());
    assert_eq!(0, NodeClass::Unspecified as u32);
}

fn do_browse(session: Arc<RwLock<Session>>, res: &[BrowseResult]) -> Result<(), Errr> {
    for BrowseResult { references, continuation_point, .. } in res {
        if let Some(references) = references {
            for r in references {
                info!(">>> reference={r:?}");

                // e.g.
                // ReferenceDescription {
                //     reference_type_id: NodeId { namespace: 0, identifier: Numeric(35) },
                //     is_forward: true,
                //     node_id: ExpandedNodeId { node_id: NodeId { namespace: 0, identifier: Numeric(85) }, namespace_uri: UAString { value: None }, server_index: 0 },
                //     browse_name: QualifiedName { namespace_index: 0, name: UAString { value: Some("Objects") } },
                //     display_name: LocalizedText { locale: UAString { value: None }, text: UAString { value: Some("Objects") } },
                //     node_class: Object,
                //     type_definition: ExpandedNodeId { node_id: NodeId { namespace: 0, identifier: Numeric(61) }, namespace_uri: UAString { value: None }, server_index: 0 }
                // }

                browse(session.clone(), r.node_id.node_id.clone())?;
            }
        }

        if !continuation_point.is_null() {
            let rlock = session.read();
            let Some(res) = rlock.browse_next(false, &[continuation_point.clone()])? else {
                continue;
            };
            do_browse(session.clone(), &res)?; // TODO: acc vec + tail call
        }
    }

    Ok(())
}
