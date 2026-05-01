#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

export KUBECONFIG="${KUBECONFIG:-/etc/rancher/k3s/k3s.yaml}"

if ! command -v helm &>/dev/null; then
  echo "==> Helm not found, installing..."
  curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash
fi

echo "==> Checking flannel MTU..."
FLANNEL_MTU=$(ip link show flannel.1 2>/dev/null | grep -oP 'mtu \K[0-9]+' || echo "0")
if [[ "$FLANNEL_MTU" -eq 0 ]]; then
  echo "  flannel.1 not found yet (k3s may still be starting)"
elif [[ "$FLANNEL_MTU" -gt 1450 ]]; then
  echo ""
  echo "ERROR: flannel.1 MTU is $FLANNEL_MTU — must be ≤1450 when running over WireGuard."
  echo "  Temporary fix:  sudo ip link set dev flannel.1 mtu 1450"
  echo "  Permanent fix:  sudo systemctl enable --now flannel-mtu-fix.service"
  echo "  (See scripts/install-k3s.sh for the service definition)"
  exit 1
else
  echo "  flannel.1 MTU is $FLANNEL_MTU — OK"
fi
echo ""

echo "==> Applying namespace"
kubectl apply -f "$REPO_ROOT/k8s/namespace.yaml"

echo "==> Deploying idle watcher (auto-shutdown after 5 min with no players)"
kubectl apply -f "$REPO_ROOT/k8s/idle-watcher/rbac.yaml"
kubectl apply -f "$REPO_ROOT/k8s/idle-watcher/state-configmap.yaml"
kubectl apply -f "$REPO_ROOT/k8s/idle-watcher/script-configmap.yaml"
kubectl apply -f "$REPO_ROOT/k8s/idle-watcher/cronjob.yaml"

echo ""
echo "==> Done!"
helm list -n games
