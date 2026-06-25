# serverctl

> Running game servers with friends shouldn't require a sysadmin.

## Motivation

Spinning up a Minecraft server for friends used to mean SSHing into a box, editing YAML, and hoping nothing broke. serverctl wraps all of that behind Discord slash commands — create, start, stop, and delete servers without leaving the chat.

## Quick Start

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

# 3. Configure the Discord bot token
cp k8s/discord-bot/secret-template.yaml k8s/discord-bot/secret.yaml
$EDITOR k8s/discord-bot/secret.yaml

# 4. Build and deploy the bot
bash scripts/install-bot.sh
```

Set up nginx on your VPS to proxy ports 25565–25568 to your home server's NodePorts:

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
| `/create <name> <type> [mods_url]` | Create and start a server. `mods_url` accepts comma-separated Modrinth, CurseForge, or direct JAR URLs. |
| `/start <name>` | Start a stopped server. |
| `/stop <name>` | Stop a running server (world data preserved). |
| `/status <name>` | Show player count, uptime, version, and connect address. |
| `/list` | List all servers in the guild. |
| `/mods <name> <url>` | Add a mod or modpack (comma-separated Modrinth/CurseForge/JAR URLs). Takes effect on next start. |
| `/curseforge-key <key>` | Save your personal CurseForge API key (required for CurseForge packs/mods). Get one at [legacy.curseforge.com/account/api-tokens](https://legacy.curseforge.com/account/api-tokens). |
| `/ttl <name> <minutes>` | Set idle-shutdown timeout. `0` disables auto-shutdown. |
| `/delete <name> [purge]` | Delete a server. `purge: true` also wipes world data. |

Servers auto-shut down after 5 minutes with no players. The idle watcher checks every 2 minutes via Minecraft's Server List Ping.

Up to **2 servers** run concurrently. Starting a third is rejected until one is stopped.

The bot replies with a "starting" embed that **updates in-place** to "ready" once the server is up.

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

If you'd like to contribute, please fork the repository and open a pull request.
