#!/usr/bin/env bash
# qemu/lib/log.sh — 统一日志函数（被各脚本 source）

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

log_info()  { echo -e "${GREEN}[✓]${NC} $*"; }
log_warn()  { echo -e "${YELLOW}[!]${NC} $*"; }
log_error() { echo -e "${RED}[✗]${NC} $*" >&2; }
log_step()  { echo -e "\n${BLUE}${BOLD}==> $*${NC}"; }
log_debug() { [ "${VERBOSE:-0}" = "1" ] && echo -e "${CYAN}[dbg]${NC} $*" || true; }
