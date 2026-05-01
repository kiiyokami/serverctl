#!/usr/bin/env bash
set -euo pipefail

NAMESPACE=games
CMD="${1:-}"
SERVER="${2:-}"

usage() {
    echo "Usage: $0 <start|stop|status|list> [server-name]"
    echo ""
    echo "Commands:"
    echo "  start  <name>   Scale deployment to 1 replica and wait for ready"
    echo "  stop   <name>   Scale deployment to 0 replicas"
    echo "  status <name>   Show deployment and pod state"
    echo "  list            Show all deployments in the games namespace"
    exit 1
}

case "$CMD" in
    start)
        [[ -z "$SERVER" ]] && usage
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
    *)
        usage
        ;;
esac
