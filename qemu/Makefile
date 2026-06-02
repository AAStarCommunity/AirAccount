# qemu/Makefile — AirAccount QEMU 开发工具链顶层入口
#
# 使用方法：
#   make -C qemu setup    # 初始化（首次）
#   make -C qemu build    # 构建 TA + CA
#   make -C qemu start    # 启动 QEMU
#   make -C qemu deploy   # 部署到 guest
#   make -C qemu test     # 运行测试
#   make -C qemu          # 完整流程：build → start → deploy
#   make -C qemu ci       # CI 模式：build → deploy（跳过 QEMU，对外部实例测试）

SHELL := /bin/bash
ROOT  := $(shell dirname $(realpath $(firstword $(MAKEFILE_LIST))))/../

.PHONY: all setup build start deploy test stop status ci help \
        build-ta build-ca clean regression

# ── 主流程 ────────────────────────────────────────────────────────────────
all: build deploy
	@echo ""
	@echo "  ✓ 构建 + 部署完成"
	@echo "  运行 'make -C qemu test' 执行集成测试"

setup:
	@$(ROOT)qemu/setup.sh

build:
	@$(ROOT)qemu/build.sh all

build-ta:
	@$(ROOT)qemu/build.sh ta

build-ca:
	@$(ROOT)qemu/build.sh ca

clean:
	@$(ROOT)qemu/build.sh clean

start:
	@$(ROOT)qemu/start.sh

start-no-tmux:
	@$(ROOT)qemu/start.sh --no-tmux

deploy:
	@$(ROOT)qemu/deploy.sh all

deploy-ca:
	@$(ROOT)qemu/deploy.sh restart

deploy-ta:
	@$(ROOT)qemu/deploy.sh ta

test:
	@$(ROOT)qemu/test.sh all

regression:
	@$(ROOT)qemu/test.sh regression

security:
	@$(ROOT)qemu/test.sh security

stop:
	@$(ROOT)qemu/stop.sh

status:
	@$(ROOT)qemu/status.sh

# ── CI 模式（无 QEMU，直接测试外部实例）────────────────────────────────
ci: build
	@echo "CI mode: testing against KMS_BASE_URL=$(KMS_BASE_URL)"
	@$(ROOT)qemu/test.sh regression

# ── 帮助 ─────────────────────────────────────────────────────────────────
help:
	@echo ""
	@echo "AirAccount QEMU 开发工具链"
	@echo ""
	@echo "命令："
	@echo "  setup          一次性初始化（拉镜像、创建目录、启动容器）"
	@echo "  build          构建 TA + CA"
	@echo "  build-ta       仅构建 TA (Trusted Application)"
	@echo "  build-ca       仅构建 CA (kms-api-server)"
	@echo "  clean          清理构建产物"
	@echo "  start          启动 QEMU（tmux 三窗格，推荐）"
	@echo "  start-no-tmux  启动 QEMU（单终端）"
	@echo "  deploy         部署 TA + CA 到 QEMU guest"
	@echo "  deploy-ca      仅重启 kms-api-server"
	@echo "  deploy-ta      仅重新加载 TA"
	@echo "  test           运行全部集成测试"
	@echo "  regression     运行快速回归测试（P0+P1+P3+P4）"
	@echo "  security       运行安全负向测试"
	@echo "  stop           停止 QEMU"
	@echo "  status         显示环境状态"
	@echo "  ci             CI 模式（直接对 KMS_BASE_URL 测试）"
	@echo ""
	@echo "完整开发周期："
	@echo "  make -C qemu setup"
	@echo "  make -C qemu start    # 在另一个终端或 tmux 里"
	@echo "  make -C qemu build"
	@echo "  make -C qemu deploy"
	@echo "  make -C qemu test"
	@echo ""
	@echo "部署验证（合并前流程）："
	@echo "  make -C qemu regression    # 快速回归"
	@echo "  make -C qemu security      # 安全检查"
	@echo ""
