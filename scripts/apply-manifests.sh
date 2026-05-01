#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CHART="$REPO_ROOT/k8s/helm/minecraft"
VALUES="$REPO_ROOT/k8s/helm/values"

if ! command -v helm &>/dev/null; then
  echo "==> Helm not found, installing..."
  curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash
fi

echo "==> Checking pod internet connectivity..."
kubectl run -n games nettest --image=busybox --restart=Never \
  -- sh -c "wget -q -O /dev/null https://launchermeta.mojang.com && echo OK || echo FAIL" 2>/dev/null || true
kubectl wait -n games pod/nettest --for=condition=Ready --timeout=30s 2>/dev/null || true
sleep 15
NETTEST_RESULT=$(kubectl logs -n games nettest 2>/dev/null || echo "FAIL")
kubectl delete pod -n games nettest 2>/dev/null || true

if [[ "$NETTEST_RESULT" != *"OK"* ]]; then
  echo ""
  echo "ERROR: Pods cannot reach the internet (HTTPS failed)."
  echo "  This is usually an MTU issue with flannel VXLAN. Fix with:"
  echo "    sudo ip link set dev flannel.1 mtu 1450"
  echo "  Then re-run this script."
  exit 1
fi
echo "  Connectivity OK"
echo ""

echo "==> Applying namespace"
kubectl apply -f "$REPO_ROOT/k8s/namespace.yaml"

echo "==> Deploying servers (replicas=0 — use scripts/server.sh start <name> to run)"
helm upgrade --install vanilla-chill "$CHART" -f "$VALUES/vanilla-chill.yaml" -n games
helm upgrade --install fabric-chill  "$CHART" -f "$VALUES/fabric-chill.yaml"  -n games
helm upgrade --install forge-chill   "$CHART" -f "$VALUES/forge-chill.yaml"   -n games

echo "==> Deploying idle watcher (auto-shutdown after 5 min with no players)"
kubectl apply -f "$REPO_ROOT/k8s/idle-watcher/rbac.yaml"
kubectl apply -f "$REPO_ROOT/k8s/idle-watcher/state-configmap.yaml"
kubectl apply -f "$REPO_ROOT/k8s/idle-watcher/script-configmap.yaml"
kubectl apply -f "$REPO_ROOT/k8s/idle-watcher/cronjob.yaml"

echo ""
echo "==> Done!"
helm list -n games
