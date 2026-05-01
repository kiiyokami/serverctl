#!/usr/bin/env bash
set -euo pipefail

echo "==> Disabling nm-cloud-setup (interferes with k3s networking on Fedora)"
sudo systemctl disable --now nm-cloud-setup.service nm-cloud-setup.timer 2>/dev/null || true

echo "==> Opening firewall ports"
sudo firewall-cmd --permanent --add-port=30000-32767/tcp
sudo firewall-cmd --permanent --add-port=6443/tcp
sudo firewall-cmd --permanent --add-port=10250/tcp

echo "==> Trusting flannel pod/service CIDRs (prevents firewalld blocking pod networking)"
sudo firewall-cmd --permanent --zone=trusted --add-source=10.42.0.0/16
sudo firewall-cmd --permanent --zone=trusted --add-source=10.43.0.0/16
sudo firewall-cmd --reload

echo "==> Installing k3s"
curl -sfL https://get.k3s.io | sh -s - --write-kubeconfig-mode 644

echo "==> Installing flannel MTU fix (prevents pod internet connectivity issues with VXLAN)"
sudo tee /etc/systemd/system/flannel-mtu-fix.service > /dev/null <<'SERVICE'
[Unit]
Description=Fix flannel.1 MTU for VXLAN over WireGuard
After=k3s.service
Requires=k3s.service

[Service]
Type=oneshot
ExecStartPre=/usr/bin/bash -c 'until ip link show flannel.1 &>/dev/null; do sleep 1; done'
ExecStart=/usr/sbin/ip link set dev flannel.1 mtu 1450
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
SERVICE
sudo systemctl daemon-reload
sudo systemctl enable --now flannel-mtu-fix.service

echo "==> Setting up kubeconfig"
mkdir -p ~/.kube
cp /etc/rancher/k3s/k3s.yaml ~/.kube/config

echo "==> Waiting for node to register..."
until kubectl get nodes 2>/dev/null | grep -q .; do sleep 2; done

echo "==> Waiting for node to be Ready"
kubectl wait node --all --for=condition=Ready --timeout=60s

echo ""
echo "==> Done! k3s is running."
kubectl get nodes
