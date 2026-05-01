#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CHART="$REPO_ROOT/k8s/helm/minecraft"
VALUES="$REPO_ROOT/k8s/helm/values/servers"
TEMPLATES="$REPO_ROOT/k8s/helm/values/templates"

export KUBECONFIG="${KUBECONFIG:-/etc/rancher/k3s/k3s.yaml}"

NAMESPACE=games
CMD="${1:-}"
SERVER="${2:-}"

usage() {
    echo "Usage: $0 <create|start|stop|status|list|delete|mods> [args]"
    echo ""
    echo "Commands:"
    echo "  create                 Interactively create a new server config and start it"
    echo "  start  <name>          Start a server (provisions it on first run)"
    echo "  stop   <name>          Scale to 0 replicas (world data preserved)"
    echo "  status <name>          Show deployment and pod state"
    echo "  list                   Show all servers"
    echo "  delete <name>          Uninstall the Helm release (world data preserved in PVC)"
    echo "  delete <name> --purge  Also delete the PVC and local values file (DESTRUCTIVE)"
    echo "  mods   <name> <url>    Add a mod or modpack from Modrinth/CurseForge or a .jar URL"
    exit 1
}

case "$CMD" in
    create)
        echo "==> New Minecraft server"
        echo ""

        # Name
        echo "Server name — used as the world identifier and Helm release name."
        echo "Keep it lowercase, no spaces (e.g. 'cobbleverse', 'survival', 'creative-test')."
        read -rp "Name: " SERVER_NAME
        if [[ -z "$SERVER_NAME" ]]; then
            echo "ERROR: Name cannot be empty."
            exit 1
        fi
        if [[ -f "$VALUES/$SERVER_NAME.yaml" ]]; then
            echo "ERROR: '$SERVER_NAME' already exists at $VALUES/$SERVER_NAME.yaml"
            exit 1
        fi

        # Type
        echo ""
        echo "Server type:"
        echo "  1) Vanilla"
        echo "  2) Fabric"
        echo "  3) Forge"
        read -rp "Choice [1-3]: " TYPE_CHOICE
        case "$TYPE_CHOICE" in
            1) SERVER_TYPE=vanilla ;;
            2) SERVER_TYPE=fabric ;;
            3) SERVER_TYPE=forge ;;
            *) echo "ERROR: Invalid choice."; exit 1 ;;
        esac

        # Auto-pick next free NodePort — check both local configs and live cluster
        LOCAL_PORTS=$(grep -h '^nodePort:' "$VALUES"/*.yaml 2>/dev/null | grep -oP '\d+' || true)
        CLUSTER_PORTS=$(kubectl get svc -n "$NAMESPACE" -o jsonpath='{.items[*].spec.ports[*].nodePort}' 2>/dev/null | tr ' ' '\n' || true)
        USED_PORTS=$(printf '%s\n%s\n' "$LOCAL_PORTS" "$CLUSTER_PORTS" | sort -u)
        NODE_PORT=""
        for PORT in $(seq 30565 30568); do
            if ! echo "$USED_PORTS" | grep -qx "$PORT"; then
                NODE_PORT=$PORT
                break
            fi
        done
        if [[ -z "$NODE_PORT" ]]; then
            echo "ERROR: All ports 30565–30568 are in use."
            exit 1
        fi
        PUBLIC_PORT=$((NODE_PORT - 5000))

        # Generate values file
        mkdir -p "$VALUES"
        sed \
            -e "s/^name:.*/name: \"$SERVER_NAME\"/" \
            -e "s/^nodePort:.*/nodePort: $NODE_PORT/" \
            "$TEMPLATES/$SERVER_TYPE.yaml" > "$VALUES/$SERVER_NAME.yaml"

        echo ""
        echo "  Created: $VALUES/$SERVER_NAME.yaml"
        echo "  Public port: $PUBLIC_PORT (connect via your domain)"
        echo ""

        read -rp "Start server now? [y/N]: " START_NOW
        if [[ "${START_NOW,,}" == "y" ]]; then
            exec "$0" start "$SERVER_NAME"
        else
            echo "  Run: bash scripts/server.sh start $SERVER_NAME"
        fi
        ;;

    start)
        [[ -z "$SERVER" ]] && usage
        VALUES_FILE="$VALUES/$SERVER.yaml"
        if [[ ! -f "$VALUES_FILE" ]]; then
            echo "ERROR: No config found for '$SERVER'."
            echo "  Run: bash scripts/server.sh create"
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
        PURGE="${3:-}"
        if [[ "$PURGE" == "--purge" ]]; then
            echo "==> PURGE: this will delete the Helm release, world data PVC, and local config."
            echo "    Server: $SERVER"
            read -rp "Type the server name to confirm: " CONFIRM
            if [[ "$CONFIRM" != "$SERVER" ]]; then
                echo "  Aborted."
                exit 1
            fi
            helm uninstall "$SERVER" -n "$NAMESPACE" 2>/dev/null || echo "  (no Helm release to uninstall)"
            kubectl delete pvc "$SERVER-data" -n "$NAMESPACE" --ignore-not-found
            rm -f "$VALUES/$SERVER.yaml"
            echo "  Purged."
        else
            echo "==> Deleting $SERVER (Helm release, deployment, and service)"
            echo "  World data in PVC '$SERVER-data' will NOT be deleted."
            helm uninstall "$SERVER" -n "$NAMESPACE"
            echo ""
            echo "  To also delete world data and local config, re-run with --purge:"
            echo "    bash scripts/server.sh delete $SERVER --purge"
        fi
        ;;

    mods)
        [[ -z "$SERVER" ]] && usage
        URL="${3:-}"
        [[ -z "$URL" ]] && usage
        VALUES_FILE="$VALUES/$SERVER.yaml"
        if [[ ! -f "$VALUES_FILE" ]]; then
            echo "ERROR: No config found for '$SERVER' at $VALUES_FILE"
            exit 1
        fi
        python3 "$SCRIPT_DIR/mod.py" "$VALUES_FILE" "$URL"
        echo ""
        if helm status "$SERVER" -n "$NAMESPACE" &>/dev/null; then
            read -rp "Apply now (helm upgrade)? [Y/n]: " APPLY
            if [[ "${APPLY,,}" != "n" ]]; then
                helm upgrade --install "$SERVER" "$CHART" -f "$VALUES_FILE" -n "$NAMESPACE"
                RUNNING=$(kubectl get deployment "$SERVER" -n "$NAMESPACE" -o jsonpath='{.spec.replicas}' 2>/dev/null || echo 0)
                if [[ "$RUNNING" -gt 0 ]]; then
                    echo "  Pod is rolling out automatically with the new config."
                else
                    echo "  Changes applied. Will take effect on next 'server.sh start $SERVER'."
                fi
            fi
        fi
        ;;

    *)
        usage
        ;;
esac
