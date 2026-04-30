#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
MANIFESTS="$REPO_ROOT/k8s/phase1"

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
