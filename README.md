# serverctl

serverctl runs Minecraft servers for a group of friends on a home k3s cluster. You create, start, stop, and delete servers with Discord slash commands instead of SSHing into a box and editing YAML.

## Quick start

### Requirements

- Fedora home server (16GB+ RAM) running k3s
- Public VPS with nginx + WireGuard tunnel to your home server
- Domain pointing to your VPS

### Install

```bash
# 1. Set up k3s on the home server
bash scripts/install-k3s.sh

# 2. Deploy the cluster (namespace, idle watcher)
bash scripts/apply-manifests.sh

# 3. Set the Discord bot token and public domain
cp k8s/discord-bot/secret-template.yaml k8s/discord-bot/secret.yaml
$EDITOR k8s/discord-bot/secret.yaml

# 4. Build and deploy the bot
bash scripts/install-bot.sh
```

Set up nginx on your VPS to proxy ports 25565-25568 to your home server's NodePorts:

```bash
sed "s/<WG_HOME_IP>/$(ip -4 -o addr show wg0 | awk '{print $4}' | cut -d/ -f1)/" \
  nginx/minecraft-stream.conf | \
  ssh root@<VPS_IP> 'cat > /etc/nginx/stream.d/minecraft.conf'
ssh root@<VPS_IP> 'nginx -t && nginx -s reload'
```

## Usage

All server management is done through Discord slash commands. Servers are scoped per guild.

| Command | Description |
|---------|-------------|
| `/create <type> [start_now]` | Opens a form for the name, Minecraft version, world seed, and mods. `type` is Vanilla, Fabric, Forge, or Paper. The version also picks the matching Java image (Java 8 through 25). Mods accepts comma-separated Modrinth/CurseForge mod or modpack URLs (a CurseForge file link such as `.../modpacks/<pack>/files/<id>` pins that version, and a server-pack file link runs the server pack), `modrinth.com/plugin/...` links (Paper only), or direct `.jar` URLs (installed as plugins on Paper). The seed is random if left blank. `start_now: false` only creates the config; start it later with `/start`. |
| `/settings <name> [gamemode] [difficulty]` | Set the default game mode and difficulty in `server.properties`, or show them when no option is given. Takes effect on next start. |
| `/start <name>` | Start a stopped server. |
| `/stop <name>` | Stop a running server (world data preserved). |
| `/status <name>` | Show player count, uptime, version, and connect address. |
| `/list` | List all servers in the guild. |
| `/mods <name> <url>` | Add mods, a modpack, or Paper plugins (comma-separated Modrinth/CurseForge/JAR URLs). Takes effect on next start. |
| `/curseforge-key <key>` | Save your personal CurseForge API key (required for CurseForge packs/mods). Get one at [console.curseforge.com](https://console.curseforge.com/); it starts with `$2a$10$`. |
| `/ttl <name> <minutes>` | Set idle-shutdown timeout. `0` disables auto-shutdown. |
| `/delete <name> [purge]` | Delete a server. `purge: true` also wipes world data. |

Servers auto-shut down after 5 minutes with no players. The idle watcher checks every 2 minutes via Minecraft's Server List Ping.

Up to 2 servers run at once. Starting a third is rejected until one is stopped.

The bot's "starting" embed updates in place to "ready" once the server is up.

## Contributing

```bash
git clone https://github.com/kiiyokami/serverctl
cd serverctl
```

Build the bot:

```bash
cd bot
cargo build
```

Deploy to k3s after changes:

```bash
bash scripts/install-bot.sh
```

To contribute, fork the repo and open a pull request.
