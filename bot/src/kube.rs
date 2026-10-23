use anyhow::Result;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{PersistentVolumeClaim, Secret, Service};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::{
    api::{Api, DeleteParams, Patch, PatchParams, PostParams},
    Client,
};
use serde_json::json;
use std::collections::BTreeMap;

const NS: &str = "games";
const CF_KEYS_SECRET: &str = "curseforge-keys";
const STOPPED_BY: &str = "serverctl.io/stopped-by";

pub async fn client() -> Result<Client> {
    Ok(Client::try_default().await?)
}

pub async fn list_deployments(c: &Client) -> Result<Vec<Deployment>> {
    let api: Api<Deployment> = Api::namespaced(c.clone(), NS);
    Ok(api.list(&Default::default()).await?.items)
}

pub async fn get_deployment(c: &Client, name: &str) -> Result<Option<Deployment>> {
    let api: Api<Deployment> = Api::namespaced(c.clone(), NS);
    match api.get(name).await {
        Ok(d) => Ok(Some(d)),
        Err(kube::Error::Api(ae)) if ae.code == 404 => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub async fn scale(c: &Client, name: &str, replicas: u32) -> Result<()> {
    let api: Api<Deployment> = Api::namespaced(c.clone(), NS);
    let patch = json!({"spec":{"replicas": replicas}});
    api.patch(name, &PatchParams::default(), &Patch::Strategic(patch))
        .await?;
    Ok(())
}

async fn patch_annotations(c: &Client, name: &str, annotations: serde_json::Value) -> Result<()> {
    let api: Api<Deployment> = Api::namespaced(c.clone(), NS);
    let patch = json!({ "metadata": { "annotations": annotations } });
    api.patch(name, &PatchParams::default(), &Patch::Merge(patch))
        .await?;
    Ok(())
}

pub async fn patch_ttl(c: &Client, name: &str, ttl_seconds: u32) -> Result<()> {
    patch_annotations(c, name, json!({ "serverctl.io/ttl-seconds": ttl_seconds.to_string() })).await
}

/// Idle shutdown delay set by /ttl; 0 means never.
pub fn ttl_secs(d: &Deployment) -> Option<u64> {
    annotation(d, "serverctl.io/ttl-seconds")?.parse().ok()
}

pub async fn running_server_count(c: &Client) -> Result<u32> {
    Ok(list_deployments(c)
        .await?
        .into_iter()
        .filter(|d| guild_id(d).is_some())
        .map(|d| replicas(&d))
        .filter(|r| *r > 0)
        .count() as u32)
}

pub async fn used_node_ports(c: &Client) -> Result<std::collections::HashSet<u32>> {
    let api: Api<Service> = Api::namespaced(c.clone(), NS);
    let svcs = api.list(&Default::default()).await?.items;
    let mut ports = std::collections::HashSet::new();
    for svc in svcs {
        if let Some(spec) = svc.spec {
            for p in spec.ports.unwrap_or_default() {
                if let Some(np) = p.node_port {
                    ports.insert(np as u32);
                }
            }
        }
    }
    Ok(ports)
}

pub async fn delete_deployment(c: &Client, name: &str) -> Result<()> {
    let api: Api<Deployment> = Api::namespaced(c.clone(), NS);
    match api.delete(name, &DeleteParams::default()).await {
        Ok(_) => Ok(()),
        Err(kube::Error::Api(ae)) if ae.code == 404 => Ok(()),
        Err(e) => Err(e.into()),
    }
}

pub async fn delete_pvc(c: &Client, name: &str) -> Result<()> {
    let api: Api<PersistentVolumeClaim> = Api::namespaced(c.clone(), NS);
    match api.delete(name, &Default::default()).await {
        Ok(_) => Ok(()),
        Err(kube::Error::Api(ae)) if ae.code == 404 => Ok(()),
        Err(e) => Err(e.into()),
    }
}

pub async fn get_curseforge_key(c: &Client, user_id: &str) -> Result<Option<String>> {
    let api: Api<Secret> = Api::namespaced(c.clone(), NS);
    match api.get(CF_KEYS_SECRET).await {
        Ok(s) => match s.data.and_then(|d| d.get(user_id).cloned()) {
            Some(bytes) => Ok(Some(String::from_utf8(bytes.0)?)),
            None => Ok(None),
        },
        Err(kube::Error::Api(ae)) if ae.code == 404 => Ok(None),
        Err(e) => Err(e.into()),
    }
}


pub async fn set_curseforge_key(c: &Client, user_id: &str, key: &str) -> Result<()> {
    let api: Api<Secret> = Api::namespaced(c.clone(), NS);
    let patch = json!({ "stringData": { user_id: key } });
    match api
        .patch(CF_KEYS_SECRET, &PatchParams::default(), &Patch::Merge(&patch))
        .await
    {
        Ok(_) => Ok(()),
        Err(kube::Error::Api(ae)) if ae.code == 404 => {
            let mut sd = BTreeMap::new();
            sd.insert(user_id.to_string(), key.to_string());
            let secret = Secret {
                metadata: ObjectMeta {
                    name: Some(CF_KEYS_SECRET.to_string()),
                    namespace: Some(NS.to_string()),
                    ..Default::default()
                },
                string_data: Some(sd),
                ..Default::default()
            };
            api.create(&PostParams::default(), &secret).await?;
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

fn annotation(d: &Deployment, key: &str) -> Option<String> {
    d.metadata.annotations.as_ref()?.get(key).cloned()
}

pub fn guild_id(d: &Deployment) -> Option<String> {
    annotation(d, "serverctl.io/discord-guild-id")
}

pub fn channel_id(d: &Deployment) -> Option<String> {
    annotation(d, "serverctl.io/discord-channel-id")
}

/// True when the last scale-down came from /stop, not idle shutdown.
pub fn stopped_by_user(d: &Deployment) -> bool {
    annotation(d, STOPPED_BY).as_deref() == Some("user")
}

/// Record which channel gets idle notices and clear the manual-stop marker.
pub async fn mark_started(c: &Client, name: &str, channel_id: &str) -> Result<()> {
    patch_annotations(
        c,
        name,
        json!({ "serverctl.io/discord-channel-id": channel_id, STOPPED_BY: null }),
    )
    .await
}

pub async fn mark_stopped_by_user(c: &Client, name: &str) -> Result<()> {
    patch_annotations(c, name, json!({ STOPPED_BY: "user" })).await
}

pub fn replicas(d: &Deployment) -> u32 {
    d.spec
        .as_ref()
        .and_then(|s| s.replicas)
        .unwrap_or(0)
        .max(0) as u32
}

pub fn uptime_secs(d: &Deployment) -> Option<i64> {
    let conditions = d.status.as_ref()?.conditions.as_ref()?;
    let t = conditions
        .iter()
        .find(|c| c.type_ == "Available" && c.status == "True")?
        .last_transition_time
        .as_ref()?;
    let now = chrono::Utc::now();
    Some(now.signed_duration_since(t.0).num_seconds().max(0))
}

pub fn available_replicas(d: &Deployment) -> u32 {
    d.status
        .as_ref()
        .and_then(|s| s.available_replicas)
        .unwrap_or(0)
        .max(0) as u32
}

pub async fn wait_until_ready(
    c: &Client,
    name: &str,
    timeout: std::time::Duration,
) -> Result<bool> {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if let Some(d) = get_deployment(c, name).await? {
            if available_replicas(&d) >= 1 {
                return Ok(true);
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
    Ok(false)
}
