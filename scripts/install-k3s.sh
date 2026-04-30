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

echo "==> Setting up kubeconfig"
mkdir -p ~/.kube
cp /etc/rancher/k3s/k3s.yaml ~/.kube/config

echo "==> Waiting for node to be Ready"
kubectl wait node --all --for=condition=Ready --timeout=60s

echo ""
echo "==> Done! k3s is running."
kubectl get nodes
