# serverctl

> Running a Minecraft server with friends shouldn't require a sysadmin.

## Hardware

Beelink SER8 — AMD Ryzen 7 8845HS, 32GB RAM, 1TB NVMe, Fedora, k3s

## Prerequisites

- A DigitalOcean droplet with nginx and WireGuard already configured to tunnel traffic to this machine
- WireGuard running on this machine (the SER8) with a stable peer IP
- A domain (`kiiyo.top`) with access to manage DNS records
- Minecraft Java Edition on the machines your friends use to connect

## Quick Start

### 1. Install k3s (run once on the SER8)

```bash
bash scripts/install-k3s.sh
```

### 2. Deploy Minecraft

```bash
bash scripts/apply-manifests.sh
```

### 3. Configure nginx on the DO droplet

Copy `nginx/minecraft-stream.conf` to `/etc/nginx/stream.d/` on your DigitalOcean droplet.

Edit the file — replace `<WG_HOME_IP>` with the WireGuard IP of the SER8 as seen from the DO droplet (check with `ip addr show wg0` on the SER8).

Ensure `/etc/nginx/nginx.conf` has a top-level `stream` block:

```nginx
stream {
    include /etc/nginx/stream.d/*.conf;
}
```

Then reload:

```bash
sudo nginx -t && sudo nginx -s reload
```

### 4. Add DNS record

In your registrar for `kiiyo.top`, add:

| Host | Type | TTL | Value |
|------|------|-----|-------|
| mc   | A    | 300 | `<DO_DROPLET_PUBLIC_IP>` |

### 5. Connect

Open Minecraft → Multiplayer → Direct Connection → `mc.kiiyo.top:25565`

## Verification

```bash
# Pod status
kubectl get pods -n games

# Follow server logs until "Done! For help, type help"
kubectl logs -n games -l app=vanilla-chill -f

# Service / NodePort
kubectl get svc -n games
```

## Adding More Servers

Each new server needs:
1. New manifests in `k8s/phase1/` with a unique `nodePort` (30566, 30567, ...)
2. A new `upstream` + `server` block in `nginx/minecraft-stream.conf` on the DO droplet
3. A new DNS A record (or friends use `mc.kiiyo.top:<port>`)
