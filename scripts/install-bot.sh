#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
export KUBECONFIG="${KUBECONFIG:-/etc/rancher/k3s/k3s.yaml}"

if [[ ! -f "$REPO_ROOT/k8s/discord-bot/secret.yaml" ]]; then
  echo "ERROR: $REPO_ROOT/k8s/discord-bot/secret.yaml is missing."
  echo "  Copy from secret-template.yaml and fill in DISCORD_TOKEN."
  exit 1
fi

if ! grep -q PUBLIC_DOMAIN "$REPO_ROOT/k8s/discord-bot/secret.yaml" \
  || grep -q '<your-' "$REPO_ROOT/k8s/discord-bot/secret.yaml"; then
  echo "ERROR: $REPO_ROOT/k8s/discord-bot/secret.yaml needs DISCORD_TOKEN and PUBLIC_DOMAIN filled in."
  echo "  See secret-template.yaml."
  exit 1
fi

echo "==> Building bot image"
cd "$REPO_ROOT/bot"
# --network host avoids docker bridge MTU issues on WireGuard hosts
docker build --network host -t serverctl-bot:dev .

echo "==> Importing image into k3s containerd"
docker save serverctl-bot:dev | sudo k3s ctr images import -

echo "==> Applying manifests"
kubectl apply -f "$REPO_ROOT/k8s/discord-bot/rbac.yaml"
kubectl apply -f "$REPO_ROOT/k8s/discord-bot/secret.yaml"
sed "s|/home/REPLACE_ME/serverctl|$REPO_ROOT|" "$REPO_ROOT/k8s/discord-bot/deployment.yaml" | kubectl apply -f -

echo "==> Restarting bot to pick up the new image"
kubectl rollout restart deployment/minecraft-discord-bot -n games
kubectl rollout status deployment/minecraft-discord-bot -n games --timeout=120s

echo ""
echo "==> Done. Recent logs:"
kubectl logs -n games -l app=minecraft-discord-bot --tail=20
