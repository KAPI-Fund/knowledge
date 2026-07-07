#!/usr/bin/env bash
# 在 KVM 主机上构建 skill-runner VM 模板镜像并注册为 CubeSandbox 模板。
# 用法: REGISTRY=<registry> TAG=<tag> ./scripts/skill-runner/build-template.sh
set -euo pipefail

REGISTRY="${REGISTRY:?set REGISTRY, e.g. myreg.example.com/knowledge}"
TAG="${TAG:-$(date +%Y%m%d-%H%M%S)}"
IMAGE="$REGISTRY/skill-runner-vm:$TAG"

# 技能目录的 git 短哈希,用于漂移核对。
SKILLS_VERSION="$(git rev-parse --short HEAD)"

echo ">> building $IMAGE (skills=$SKILLS_VERSION)"
docker build \
  -f Dockerfile.skill-runner-vm \
  --build-arg "SKILLS_VERSION=$SKILLS_VERSION" \
  -t "$IMAGE" .

echo ">> pushing $IMAGE"
docker push "$IMAGE"

echo ">> registering CubeSandbox template"
# --expose-port/--probe 端口按 CubeSandbox 部署实际调整。
cubemastercli tpl create-from-image \
  --image "$IMAGE" \
  --writable-layer-size 1G \
  --expose-port 49999 \
  --probe 49999

echo ">> done. 取输出里的 template_id,填进 skill-runner 的 CUBE_TEMPLATE_ID。"
