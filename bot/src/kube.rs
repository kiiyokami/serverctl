use anyhow::Result;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::PersistentVolumeClaim;
use kube::{
    api::{Api, Patch, PatchParams},
    Client,
};
use serde_json::json;

const NS: &str = "games";

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

pub async fn delete_pvc(c: &Client, name: &str) -> Result<()> {
    let api: Api<PersistentVolumeClaim> = Api::namespaced(c.clone(), NS);
    match api.delete(name, &Default::default()).await {
        Ok(_) => Ok(()),
        Err(kube::Error::Api(ae)) if ae.code == 404 => Ok(()), // already gone
        Err(e) => Err(e.into()),
    }
}

pub fn guild_id(d: &Deployment) -> Option<String> {
    d.metadata
        .annotations
        .as_ref()?
        .get("serverctl.io/discord-guild-id")
        .cloned()
}

pub fn replicas(d: &Deployment) -> u32 {
    d.spec
        .as_ref()
        .and_then(|s| s.replicas)
        .unwrap_or(0)
        .max(0) as u32
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
