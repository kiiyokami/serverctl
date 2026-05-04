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

| Port  | NodePort |
|-------|----------|
| 25565 | 30565    |
| 25566 | 30566    |
| 25567 | 30567    |
| 25568 | 30568    |

Server configs live locally in `k8s/helm/values/servers/` (gitignored — each machine manages its own).

At most **2 servers run concurrently** (hardware limit; configurable as `MAX_CONCURRENT` in the bot source). Trying to start a third while two are running is rejected with an explanation.

Servers auto-shut-down after idle. The `minecraft-idle-watcher` CronJob checks every 2 minutes via Minecraft's Server List Ping and scales the deployment to 0 when empty. The default TTL is 5 minutes; use `/ttl` to override per server.

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

### 3. Set up the Discord bot

1. Create a Discord application + bot at https://discord.com/developers/applications, copy the bot token.
2. Copy the secret template and fill in your token:
   ```bash
   cp k8s/discord-bot/secret-template.yaml k8s/discord-bot/secret.yaml
   $EDITOR k8s/discord-bot/secret.yaml
   ```
3. Edit `k8s/discord-bot/deployment.yaml` — replace the `REPLACE_ME` placeholder in `hostPath.path` with the absolute path of this repo on your home server.
4. Build and install:
   ```bash
   bash scripts/install-bot.sh
   ```
5. Invite the bot to your Discord with the `applications.commands` and `bot` scopes.

### 4. Configure nginx on the VPS (one-time setup)

Copy `nginx/minecraft-stream.conf` to the VPS, replacing `<WG_HOME_IP>` with your home server's WireGuard IP:

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

## Discord Bot Commands

All server management is done through Discord slash commands. Servers are scoped per guild — each guild only sees and manages servers it created.

### `/create <name> <type> [mods_url]`

Creates and starts a server. `mods_url` accepts a single URL or multiple comma-separated URLs:

- Modrinth modpack: `https://modrinth.com/modpack/cobbleverse`
- Modrinth mod: `https://modrinth.com/mod/lithium`
- Direct JAR: `https://example.com/mod.jar`
- Multiple: `https://modrinth.com/mod/lithium,https://modrinth.com/mod/sodium`

The bot replies with a "starting" embed that updates in-place to "ready" when the server is up, including the connect address.

### `/start <name>`

Starts a stopped server. Updates in-place from "starting" to "ready".

### `/stop <name>`

Scales the server to 0. World data is preserved.

### `/status <name>`

Shows current state with player count, uptime, version, and connect address.

### `/list`

Lists all servers in the guild with their status, player count, uptime, and version.

### `/mods <name> <url>`

Adds a mod or modpack to an existing server. Accepts comma-separated URLs. Takes effect on next start.

### `/ttl <name> <minutes>`

Sets the idle-shutdown timeout for a specific server. Set to `0` to disable auto-shutdown. Default is 5 minutes.

### `/delete <name> [purge]`

Removes the server. Without `purge`, world data (PVC) is kept. With `purge: true`, everything including the world is deleted.

## Server Configuration

Each server's `k8s/helm/values/servers/<name>.yaml` controls everything:

```yaml
name: "my-server"
nodePort: 30565

# Container image — pick one to match your mod/loader:
#   itzg/minecraft-server:latest   — newest Java (currently 25)
#   itzg/minecraft-server:java21   — Java 21 (most 1.20+ mods)
#   itzg/minecraft-server:java17   — Java 17 (1.17–1.20)
image: itzg/minecraft-server:java21

server:
  type: FABRIC                  # VANILLA | FABRIC | FORGE
  version: "1.21.1"             # Pin a specific MC version. Avoid LATEST — newer
                                # MC may need newer Java than your image provides.
  memory: "8G"
  onlineMode: true
  mods: []                      # individual mod JAR URLs (managed by /mods)

resources:
  requests: { memory: "8Gi", cpu: "1" }
  limits:   { memory: "12Gi", cpu: "4" }

storage: 40Gi

# Extra env vars passed straight to itzg/minecraft-server.
# Used for modpacks (TYPE=MODRINTH, MODRINTH_PROJECT) and tunables like MOTD, MAX_PLAYERS, etc.
extraEnv:
  JVM_OPTS: "-XX:+UseZGC -XX:+ZGenerational"
```

## DNS (one-time setup)

One A record covers all servers — they share the domain, just different ports.

| Host | Type | Value              |
|------|------|--------------------|
| mc   | A    | `<VPS_PUBLIC_IP>`  |

## Tips

- **`kubectl` and `helm` outside the scripts**: k3s stores its kubeconfig at `/etc/rancher/k3s/k3s.yaml`, which `helm` doesn't auto-discover. Add this to your `~/.bashrc`:
  ```bash
  export KUBECONFIG=/etc/rancher/k3s/k3s.yaml
  ```
- **Modpacks take time to boot.** First start can run 5–10 minutes while the modpack downloads and dimensions generate. The bot's "starting" message updates in-place when it's actually ready.
- **Java version matters.** Minecraft 1.21.5+ requires Java 25. Use `itzg/minecraft-server:java21` for 1.21.4 and below.
- **World data persists** across stops, deletes (without `purge`), and pod restarts via the PVC.
- **Idle shutdown is per-server.** Use `/ttl <name> 0` to keep a server running indefinitely.

## Verification

```bash
helm list -n games                                  # all releases
kubectl get deployments -n games                    # all servers
kubectl logs -n games -l app=<name> -f              # follow server logs
# wait for: [Server thread/INFO]: Done (Xs)! For help, type "help"

kubectl get cronjob minecraft-idle-watcher -n games  # idle watcher healthy
kubectl logs -n games -l job-name --tail=50          # recent watcher runs
```
