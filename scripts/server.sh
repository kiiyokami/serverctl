#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CHART="$REPO_ROOT/k8s/helm/minecraft"
VALUES="$REPO_ROOT/k8s/helm/values"

export KUBECONFIG="${KUBECONFIG:-/etc/rancher/k3s/k3s.yaml}"

NAMESPACE=games
CMD="${1:-}"
SERVER="${2:-}"

usage() {
    echo "Usage: $0 <start|stop|status|list|delete> [server-name]"
    echo ""
    echo "Commands:"
    echo "  start  <name>   Provision and start a server (creates it if it doesn't exist)"
    echo "  stop   <name>   Scale deployment to 0 replicas (world data preserved)"
    echo "  status <name>   Show deployment and pod state"
    echo "  list            Show all deployments in the games namespace"
    echo "  delete <name>   Uninstall the Helm release (world data preserved in PVC)"
    exit 1
}

case "$CMD" in
    start)
        [[ -z "$SERVER" ]] && usage
        VALUES_FILE="$VALUES/$SERVER.yaml"
        if [[ ! -f "$VALUES_FILE" ]]; then
            echo "ERROR: No values file found at $VALUES_FILE"
            echo "  Create one to define the server configuration."
            exit 1
        fi
        if ! helm status "$SERVER" -n "$NAMESPACE" &>/dev/null; then
            echo "==> Provisioning $SERVER for the first time..."
            helm upgrade --install "$SERVER" "$CHART" -f "$VALUES_FILE" -n "$NAMESPACE"
        fi
        echo "==> Starting $SERVER"
        kubectl scale deployment/"$SERVER" -n "$NAMESPACE" --replicas=1
        echo "==> Waiting for $SERVER to be ready (up to 3 minutes)..."
        kubectl rollout status deployment/"$SERVER" -n "$NAMESPACE" --timeout=180s
        echo ""
        kubectl get pods -n "$NAMESPACE" -l "app=$SERVER"
        ;;
    stop)
        [[ -z "$SERVER" ]] && usage
        echo "==> Stopping $SERVER"
        kubectl scale deployment/"$SERVER" -n "$NAMESPACE" --replicas=0
        echo "  Stopped. World data is safe on the PVC."
        ;;
    status)
        [[ -z "$SERVER" ]] && usage
        kubectl get deployment/"$SERVER" -n "$NAMESPACE"
        echo ""
        kubectl get pods -n "$NAMESPACE" -l "app=$SERVER"
        ;;
    list)
        kubectl get deployments -n "$NAMESPACE"
        ;;
    delete)
        [[ -z "$SERVER" ]] && usage
        echo "==> Deleting $SERVER (Helm release, deployment, and service)"
        echo "  World data in PVC '$SERVER-data' will NOT be deleted."
        helm uninstall "$SERVER" -n "$NAMESPACE"
        echo ""
        echo "  To also delete world data:"
        echo "    kubectl delete pvc $SERVER-data -n $NAMESPACE"
        ;;
    *)
        usage
        ;;
esac
