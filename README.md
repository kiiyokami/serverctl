# serverctl

> Running a Minecraft server with friends shouldn't require a sysadmin.

## Prerequisites

- A server (home rig or VPS) running Fedora with at least 8GB RAM
- A public-facing VPS with nginx and WireGuard already configured to tunnel traffic to your home server
- WireGuard running on your home server with a stable peer IP
- A domain with access to manage DNS records
- Minecraft Java Edition on the machines your friends use to connect

## Quick Start

### 1. Install k3s (run once on your home server)

```bash
bash scripts/install-k3s.sh
```

### 2. Deploy Minecraft

```bash
bash scripts/apply-manifests.sh
```

### 3. Configure nginx on the VPS

Copy `nginx/minecraft-stream.conf` to `/etc/nginx/stream.d/` on your public VPS.

Edit the file — replace `<WG_HOME_IP>` with the WireGuard IP of your home server as seen from the VPS (check with `ip addr show wg0` on the home server).

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

### 4. DNS

If you have a wildcard DNS record (`*.yourdomain.com → <VPS_PUBLIC_IP>`), no changes needed — `mc.yourdomain.com` resolves automatically.

Otherwise add an A record in your registrar:

| Host | Type | TTL | Value |
|------|------|-----|-------|
| mc   | A    | 300 | `<VPS_PUBLIC_IP>` |

### 5. Connect

Open Minecraft → Multiplayer → Direct Connection → `mc.yourdomain.com:25565`

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
2. A new `upstream` + `server` block in `nginx/minecraft-stream.conf` on the VPS
3. If not using a wildcard, a new DNS A record (or friends use `mc.yourdomain.com:<port>`)
