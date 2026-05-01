# serverctl

> Running game servers with friends shouldn't require a sysadmin.

## Hardware

Beelink SER8 — AMD Ryzen 7 8845HS, 32GB RAM, 1TB NVMe, Fedora, k3s

## Prerequisites

- Fedora server (home rig) with at least 16GB RAM
- A public-facing VPS with nginx and WireGuard already configured to tunnel traffic to your home server
- WireGuard running on your home server with a stable peer IP
- A domain with access to manage DNS records
- Minecraft Java Edition on the machines your friends use to connect

## Servers

| Server | Type | Address | NodePort |
|--------|------|---------|----------|
| vanilla-chill | Vanilla | `mc.kiiyo.top:25565` | 30565 |
| fabric-chill  | Fabric  | `mc-fabric.kiiyo.top:25566` | 30566 |
| forge-chill   | Forge   | `mc-forge.kiiyo.top:25567` | 30567 |

All servers default to `replicas: 0` — deployed but not running. Use `scripts/server.sh` to start them on demand.

Servers shut down automatically after **5 minutes with no players** (idle TTL). The `minecraft-idle-watcher` CronJob checks every 2 minutes via Minecraft's Server List Ping and scales the deployment to 0 when the server has been empty long enough. To change the TTL, edit `TTL_SECONDS` in [k8s/idle-watcher/cronjob.yaml](k8s/idle-watcher/cronjob.yaml) and re-run `apply-manifests.sh`.

## Quick Start

### 1. Install k3s (run once on the SER8)

```bash
bash scripts/install-k3s.sh
```

### 2. Set up the cluster

```bash
bash scripts/apply-manifests.sh
```

Creates the `games` namespace and deploys the idle watcher. No Minecraft servers are created yet.

### 3. Start a server

```bash
bash scripts/server.sh start vanilla-chill
```

On first run this provisions the Helm release (deployment, service, PVC) and starts it. Subsequent starts just scale it up.

### 4. Stop a server

```bash
bash scripts/server.sh stop vanilla-chill
```

### 5. Delete a server

```bash
bash scripts/server.sh delete vanilla-chill
```

Removes the Helm release. World data in the PVC is preserved. To also wipe the world:

```bash
kubectl delete pvc vanilla-chill-data -n games
```

### 6. Check server status

```bash
bash scripts/server.sh status vanilla-chill
bash scripts/server.sh list
```

## Configure nginx on the VPS

Add stream blocks to `/etc/nginx/stream.d/minecraft.conf` on the VPS for each server. The `listen` port is what friends connect to publicly; the `server` address is the WireGuard IP of the SER8 plus the k3s NodePort.

```nginx
upstream minecraft-vanilla-chill {
    server 10.66.66.2:30565;
}
server {
    listen 25565;
    proxy_pass minecraft-vanilla-chill;
    proxy_timeout 600s;
    proxy_connect_timeout 10s;
}

upstream minecraft-fabric-chill {
    server 10.66.66.2:30566;
}
server {
    listen 25566;
    proxy_pass minecraft-fabric-chill;
    proxy_timeout 600s;
    proxy_connect_timeout 10s;
}

upstream minecraft-forge-chill {
    server 10.66.66.2:30567;
}
server {
    listen 25567;
    proxy_pass minecraft-forge-chill;
    proxy_timeout 600s;
    proxy_connect_timeout 10s;
}
```

Open the new ports in UFW and reload nginx:

```bash
sudo ufw allow 25566/tcp
sudo ufw allow 25567/tcp
sudo nginx -s reload
```

## DNS

Add A records pointing to the VPS public IP:

| Host | Type | Value |
|------|------|-------|
| mc | A | `<VPS_PUBLIC_IP>` |
| mc-fabric | A | `<VPS_PUBLIC_IP>` |
| mc-forge | A | `<VPS_PUBLIC_IP>` |

## Adding Mods

Edit the `mods` list in the server's values file:

```yaml
# k8s/helm/values/fabric-chill.yaml
server:
  mods:
    - https://cdn.modrinth.com/data/AANobbMI/versions/IZskON6d/sodium-fabric-0.5.8%2Bmc1.21.jar
    - https://cdn.modrinth.com/data/gvQqBUqZ/versions/oKSNd6ca/lithium-fabric-mc1.21-0.12.1.jar
```

Then apply the update:

```bash
helm upgrade fabric-chill k8s/helm/minecraft -f k8s/helm/values/fabric-chill.yaml -n games
```

The pod restarts automatically and downloads the mods on startup.

## Adding a New Server

1. Create `k8s/helm/values/<server-name>.yaml` with a unique `name` and `nodePort`
2. Add a `helm upgrade --install` line for it in `scripts/apply-manifests.sh`
3. Add a stream block in `/etc/nginx/stream.d/minecraft.conf` on the VPS
4. Open the port in UFW and reload nginx
5. Add a DNS A record if using a subdomain

## Verification

```bash
# All releases installed
helm list -n games

# All deployments at 0 replicas
kubectl get deployments -n games

# Start vanilla and follow logs
bash scripts/server.sh start vanilla-chill
kubectl logs -n games -l app=vanilla-chill -f
# Wait for: [Server thread/INFO]: Done (Xs)! For help, type "help"

# Connect: mc.kiiyo.top:25565

# Check idle watcher is running
kubectl get cronjob minecraft-idle-watcher -n games

# View idle watcher logs (runs every 2 min)
kubectl logs -n games -l job-name --tail=50
```
