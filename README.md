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

| Port | NodePort |
|------|----------|
| 25565 | 30565 |
| 25566 | 30566 |
| 25567 | 30567 |
| 25568 | 30568 |

Server configs live locally in `k8s/helm/values/servers/` (gitignored — each machine manages its own).

At most **2 servers run concurrently** (hardware limit; configurable as `MAX_CONCURRENT` in `scripts/server.sh`). Trying to `start` a third while two are running fails with a list of currently-running servers to stop first.

Servers auto-shut-down after **5 minutes with no players**. The `minecraft-idle-watcher` CronJob checks every 2 minutes via Minecraft's Server List Ping and scales the deployment to 0 when empty. Change the TTL by editing `TTL_SECONDS` in [k8s/idle-watcher/cronjob.yaml](k8s/idle-watcher/cronjob.yaml) and re-running `apply-manifests.sh`.

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

### 3. Create a server

```bash
bash scripts/server.sh create
```

Prompts for:
- **Name** — used as the world identifier and Helm release name (lowercase, no spaces)
- **Type** — Vanilla, Fabric, or Forge

Auto-picks the next free NodePort, generates the config in `k8s/helm/values/servers/<name>.yaml`, and offers to start it immediately.

### 4. Add a mod or modpack (optional)

```bash
# Modrinth modpack
bash scripts/server.sh mods <name> https://modrinth.com/modpack/cobbleverse

# Modrinth single mod (auto-resolves the version matching your server's MC version)
bash scripts/server.sh mods <name> https://modrinth.com/mod/lithium

# CurseForge modpack (requires CF_API_KEY — see below)
bash scripts/server.sh mods <name> https://www.curseforge.com/minecraft/modpacks/all-the-mods-9

# Direct JAR URL
bash scripts/server.sh mods <name> https://example.com/path/to/mod.jar
```

If the server is already deployed, the script prompts to apply via `helm upgrade` (Kubernetes rolls the pod automatically).

### 5. Start, stop, status

```bash
bash scripts/server.sh start <name>     # provisions on first run, then scales to 1
bash scripts/server.sh stop <name>      # scales to 0 (world data preserved)
bash scripts/server.sh status <name>    # show deployment + pod state
bash scripts/server.sh list             # all servers in the games namespace
```

### 6. Delete a server

```bash
bash scripts/server.sh delete <name>          # uninstall Helm release (PVC kept)
bash scripts/server.sh delete <name> --purge  # also wipe world data + local config
```

`--purge` requires you to type the server name to confirm.

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
  mods: []                      # individual mod JAR URLs (managed by `server.sh mods`)

resources:
  requests: { memory: "8Gi", cpu: "1" }
  limits:   { memory: "12Gi", cpu: "4" }

storage: 40Gi

# Extra env vars passed straight to itzg/minecraft-server. Used by `server.sh mods`
# to set TYPE=MODRINTH/MODRINTH_PROJECT for modpacks. Also useful for tunables like
# JVM_OPTS, MOTD, MAX_PLAYERS, etc.
extraEnv:
  JVM_OPTS: "-XX:+UseZGC -XX:+ZGenerational"
```

### CurseForge API key

CurseForge modpacks require an API key. Get one at https://console.curseforge.com/, then add it to the values file:

```yaml
extraEnv:
  TYPE: AUTO_CURSEFORGE
  CF_SLUG: <pack-slug>
  CF_API_KEY: "<your-key>"
```

The `servers/` directory is gitignored, so the key stays local.

## Configure nginx on the VPS (one-time setup)

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

## Discord Bot (optional)

Rust bot that exposes server lifecycle as slash commands. Servers are scoped per Discord guild — each guild only sees and manages servers it created. Servers created via `scripts/server.sh` directly (no `discordGuildId`) are invisible to the bot.

### Setup

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

### Commands

`/create <name> <type>`, `/list`, `/status <name>`, `/start <name>`, `/stop <name>`, `/delete <name> [purge:true]`, `/mods <name> <url>`

`MAX_CONCURRENT=2` is enforced in `/start` (matches the host script).

## DNS (one-time setup)

One A record covers all servers — they share the domain, just different ports.

| Host | Type | Value |
|------|------|-------|
| mc | A | `<VPS_PUBLIC_IP>` |

## Tips

- **`kubectl` and `helm` outside the scripts**: k3s stores its kubeconfig at `/etc/rancher/k3s/k3s.yaml`, which `helm` doesn't auto-discover. Add this to your `~/.bashrc`:
  ```bash
  export KUBECONFIG=/etc/rancher/k3s/k3s.yaml
  ```
- **Modpacks take time to boot.** First start can run 5–10 minutes while it downloads the modpack and generates dimensions. The startup probe gives it up to 10 minutes before the pod is considered failed.
- **World data persists** across stops, deletes (without `--purge`), and pod restarts via the PVC.

## Verification

```bash
helm list -n games                                  # all releases
kubectl get deployments -n games                    # all servers
kubectl logs -n games -l app=<name> -f              # follow server logs
# wait for: [Server thread/INFO]: Done (Xs)! For help, type "help"

kubectl get cronjob minecraft-idle-watcher -n games  # idle watcher healthy
kubectl logs -n games -l job-name --tail=50          # recent watcher runs
```
