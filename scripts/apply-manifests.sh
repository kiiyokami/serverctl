#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
MANIFESTS="$REPO_ROOT/k8s/phase1"

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

echo "==> Applying Phase 1 manifests"
kubectl apply -f "$MANIFESTS/namespace.yaml"
kubectl apply -f "$MANIFESTS/pvc.yaml"
kubectl apply -f "$MANIFESTS/deployment.yaml"
kubectl apply -f "$MANIFESTS/service.yaml"

echo "==> Waiting for deployment to be available (up to 3 minutes)..."
kubectl wait deployment/vanilla-chill \
  --namespace=games \
  --for=condition=Available \
  --timeout=180s

echo ""
echo "==> Done!"
echo ""
echo "Pods:"
kubectl get pods -n games
echo ""
echo "Services:"
kubectl get svc -n games
