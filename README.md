# serverctl

> Running game servers with friends shouldn't require a sysadmin.


## Prerequisites

- Fedora server (home rig) with at least 16GB RAM
- A public-facing VPS with nginx and WireGuard already configured to tunnel traffic to your home server
- WireGuard running on your home server with a stable peer IP
- A domain with access to manage DNS records
- Minecraft Java Edition on the machines your friends use to connect

## Servers

All servers live at your domain on different ports (e.g. `mc.example.com:25565`, `mc.example.com:25566`):

| Port | NodePort | Notes |
|------|----------|-------|
| 25565 | 30565 | |
| 25566 | 30566 | |
| 25567 | 30567 | |
| 25568 | 30568 | |

Servers are defined locally in `k8s/helm/values/servers/` (gitignored — each machine manages its own). Copy a template from `k8s/helm/values/templates/` to get started.

Servers shut down automatically after **5 minutes with no players** (idle TTL). The `minecraft-idle-watcher` CronJob checks every 2 minutes via Minecraft's Server List Ping and scales the deployment to 0 when the server has been empty long enough. To change the TTL, edit `TTL_SECONDS` in [k8s/idle-watcher/cronjob.yaml](k8s/idle-watcher/cronjob.yaml) and re-run `apply-manifests.sh`.

## Quick Start

### 1. Install k3s (run once on the home server)

```bash
bash scripts/install-k3s.sh
```

### 2. Set up the cluster

```bash
bash scripts/apply-manifests.sh
```

Creates the `games` namespace and deploys the idle watcher. No Minecraft servers are created yet.

### 3. Create and start a server

```bash
bash scripts/server.sh create
```

Prompts for a name and type (vanilla/fabric/forge), auto-picks the next free port, generates the config in `k8s/helm/values/servers/`, and offers to start it immediately.

### 5. Stop a server

```bash
bash scripts/server.sh stop my-server
```

### 6. Delete a server

```bash
bash scripts/server.sh delete my-server
```

Removes the Helm release. World data in the PVC is preserved. To also wipe the world:

```bash
kubectl delete pvc my-server-data -n games
```

### 7. Check server status

```bash
bash scripts/server.sh status my-server
bash scripts/server.sh list
```

## Configure nginx on the VPS (one-time setup)

Copy `nginx/minecraft-stream.conf` to the VPS, replacing `<WG_HOME_IP>` with the WireGuard IP of your home server. Then open the port range. You never need to touch this again — all 4 slots are pre-wired.

```bash
sed "s/<WG_HOME_IP>/$(ip -4 -o addr show wg0 | awk '{print $4}' | cut -d/ -f1)/" \
  nginx/minecraft-stream.conf | \
  ssh root@<VPS_IP> 'cat > /etc/nginx/stream.d/minecraft.conf'
```

On the VPS:

```bash
sudo ufw allow 25565:25568/tcp
sudo nginx -t && sudo nginx -s reload
```

Make sure `/etc/nginx/nginx.conf` has a top-level `stream` block:

```nginx
stream {
    include /etc/nginx/stream.d/*.conf;
}
```

## DNS (one-time setup)

One A record is all you need. All servers share the same domain, different ports.

| Host | Type | Value |
|------|------|-------|
| mc | A | `<VPS_PUBLIC_IP>` |

## Adding Mods

Edit the `mods` list in your server's values file:

```yaml
server:
  mods:
    - https://cdn.modrinth.com/data/AANobbMI/versions/IZskON6d/sodium-fabric-0.5.8%2Bmc1.21.jar
    - https://cdn.modrinth.com/data/gvQqBUqZ/versions/oKSNd6ca/lithium-fabric-mc1.21-0.12.1.jar
```

Then apply the update:

```bash
helm upgrade <name> k8s/helm/minecraft -f k8s/helm/values/servers/<name>.yaml -n games
```

The pod restarts automatically and downloads the mods on startup.

## Adding a New Server

1. Copy a template: `cp k8s/helm/values/templates/forge.yaml k8s/helm/values/servers/<name>.yaml`
2. Set `name` and pick an unused `nodePort` from 30565–30568
3. Run `bash scripts/server.sh start <name>`

That's it. nginx and DNS are already configured.

## Verification

```bash
# All releases installed
helm list -n games

# All deployments at 0 replicas
kubectl get deployments -n games

# Create and start a server
bash scripts/server.sh create

# Follow logs
kubectl logs -n games -l app=<name> -f
# Wait for: [Server thread/INFO]: Done (Xs)! For help, type "help"

# Check idle watcher is running
kubectl get cronjob minecraft-idle-watcher -n games

# View idle watcher logs (runs every 2 min)
kubectl logs -n games -l job-name --tail=50
```
