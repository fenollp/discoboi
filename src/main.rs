use std::{collections::HashMap, env, str::FromStr, sync::Arc};

use env_logger::Env;
use log::{debug, info, warn};
use opcua::{
    client::prelude::{Client, ClientConfig, IdentityToken, Session, ViewService},
    core::comms::url::is_opc_ua_binary_url,
    crypto::SecurityPolicy,
    sync::RwLock,
    types::{
        ApplicationDescription, BrowseDescription, BrowseDescriptionResultMask, BrowseDirection,
        BrowseResult, EndpointDescription, MessageSecurityMode, NodeClass, NodeClassMask, NodeId,
        ReferenceDescription, ReferenceTypeId, UserTokenPolicy,
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
                browse(session)?;
            }
        }
    }

    Ok(())
}

type Nodes = HashMap<NodeId, (NodeClass, String, NodeId)>;

fn browse(session: Arc<RwLock<Session>>) -> Result<(), Errr> {
    let mut map = Default::default();
    browse_node(session.clone(), NodeId::root_folder_id(), &mut map)?;

    info!(">>>> Found {} nodes:", map.len());
    for (node_id, (class, name, ty)) in map {
        info!(">>>> Node: {class:?} {name} {node_id} -> {ty}");
    }
    Ok(())
}

fn browse_node(
    session: Arc<RwLock<Session>>,
    node_id: NodeId,
    map: &mut Nodes,
) -> Result<(), Errr> {
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

    do_browse(session.clone(), &res, map)
}

fn do_browse(
    session: Arc<RwLock<Session>>,
    res: &[BrowseResult],
    map: &mut Nodes,
) -> Result<(), Errr> {
    let continuation_points: Vec<_> = res
        .iter()
        .map(|BrowseResult { continuation_point, .. }| continuation_point.clone())
        .filter(|cp| !cp.is_null())
        .collect();

    let nodes: HashMap<_, _> = res
        .iter()
        .flat_map(|BrowseResult { references, .. }| {
            references.clone().unwrap_or_default().into_iter()
        })
        .inspect(|reference| debug!(">>> reference {}", RefDescr(reference)))
        .map(
            |ReferenceDescription { node_id, browse_name, node_class, type_definition, .. }| {
                let name = browse_name.name.value().clone().unwrap_or_default();
                (node_id.node_id.clone(), (node_class, name, type_definition.node_id.clone()))
            },
        )
        .collect();

    if !continuation_points.is_empty() {
        let rlock = session.read();
        if let Some(res) = rlock.browse_next(false, &continuation_points)? {
            do_browse(session.clone(), &res, map)?;
        }
    }

    info!("about to browse {}/{} nodes", nodes.len(), map.len());
    // TODO: Stream iter
    for (node_id, node_value) in nodes {
        if map.insert(node_id.clone(), node_value).is_none() {
            debug!("browsing node {node_id}");
            browse_node(session.clone(), node_id.clone(), map)?;
            debug!("browsed node {node_id}");
        }
    }

    Ok(())
}

#[test]
fn wth_typing_exists_for_a_reason() {
    use opcua::types::{BrowseResultMask, NodeClass};

    assert_eq!(63, BrowseDescriptionResultMask::all().bits());
    assert_eq!(63, BrowseResultMask::All as u32);
    assert_eq!(255, NodeClassMask::all().bits());
    assert_eq!(0, NodeClass::Unspecified as u32);
}

struct RefDescr<'a>(&'a ReferenceDescription);

impl<'a> std::fmt::Display for RefDescr<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {:?} {:?} (-> {}) ",
            self.0.node_id.node_id,
            self.0.node_class,
            self.0.browse_name.name.value(),
            self.0.type_definition.node_id
        )
    }
}
