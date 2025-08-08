#!/bin/bash
# AirAccount 项目通用脚本库
# 提供统一的错误处理、环境检查、配置加载等功能

set -euo pipefail

# 颜色定义
readonly RED='\033[0;31m'
readonly GREEN='\033[0;32m'
readonly YELLOW='\033[1;33m'
readonly BLUE='\033[0;34m'
readonly NC='\033[0m' # No Color

# 项目根目录 (自动检测)
readonly PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

# 日志函数
log_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

log_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

log_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

log_error() {
    echo -e "${RED}❌ $1${NC}" >&2
}

# 统一错误处理函数
handle_error() {
    local error_msg="${1:-未知错误}"
    local exit_code="${2:-1}"
    
    log_error "Error: $error_msg"
    log_error "脚本执行失败，退出码: $exit_code"
    log_error "请检查上述错误信息并重试"
    
    exit "$exit_code"
}

# 设置错误陷阱
trap 'handle_error "脚本在第 $LINENO 行执行失败" $?' ERR

# 环境检查函数
check_os() {
    case "$OSTYPE" in
        darwin*)
            log_success "macOS环境检测完成"
            return 0
            ;;
        linux-gnu*)
            log_success "Linux环境检测完成"
            return 0
            ;;
        *)
            handle_error "不支持的操作系统: $OSTYPE，支持 macOS 或 Linux"
            ;;
    esac
}

check_docker() {
    if command -v docker &> /dev/null; then
        if docker info &> /dev/null; then
            log_success "Docker环境检查通过: $(docker --version | cut -d' ' -f3 | tr -d ',')"
            return 0
        else
            handle_error "Docker daemon未运行，请启动Docker服务"
        fi
    else
        handle_error "Docker未安装，请先安装Docker"
    fi
}

check_git() {
    if command -v git &> /dev/null; then
        log_success "Git检查通过: $(git --version)"
        return 0
    else
        handle_error "Git未安装，请先安装Git"
    fi
}

check_rust() {
    if command -v rustc &> /dev/null && command -v cargo &> /dev/null; then
        log_success "Rust工具链检查通过: $(rustc --version)"
        return 0
    else
        handle_error "Rust工具链未安装，请先运行 ./scripts/setup_rust.sh"
    fi
}

# 配置加载函数
load_config() {
    local config_file="${1:-$PROJECT_ROOT/config/development.conf}"
    
    if [[ ! -f "$config_file" ]]; then
        handle_error "配置文件不存在: $config_file"
    fi
    
    # 设置AIRACCOUNT_ROOT环境变量供配置文件使用
    export AIRACCOUNT_ROOT="$PROJECT_ROOT"
    
    # 安全地加载配置文件
    set +u # 暂时允许未定义变量
    source "$config_file"
    set -u
    
    log_success "配置文件加载完成: $config_file"
}

# 验证必需的环境变量
validate_env_vars() {
    local required_vars=("$@")
    local missing_vars=()
    
    for var in "${required_vars[@]}"; do
        if [[ -z "${!var:-}" ]]; then
            missing_vars+=("$var")
        fi
    done
    
    if [[ ${#missing_vars[@]} -gt 0 ]]; then
        handle_error "缺少必需的环境变量: ${missing_vars[*]}"
    fi
    
    log_success "环境变量验证通过"
}

# 目录检查和创建
ensure_directory() {
    local dir_path="$1"
    
    if [[ ! -d "$dir_path" ]]; then
        log_info "创建目录: $dir_path"
        mkdir -p "$dir_path" || handle_error "无法创建目录: $dir_path"
    fi
    
    log_success "目录检查通过: $dir_path"
}

# 文件存在性检查
check_file_exists() {
    local file_path="$1"
    local description="${2:-文件}"
    
    if [[ ! -f "$file_path" ]]; then
        handle_error "$description 不存在: $file_path"
    fi
    
    log_success "$description 检查通过: $file_path"
}

# 网络连接检查
check_internet() {
    local test_url="${1:-https://github.com}"
    
    if curl -s --head --connect-timeout 5 "$test_url" > /dev/null; then
        log_success "网络连接检查通过"
        return 0
    else
        handle_error "网络连接失败，无法访问: $test_url"
    fi
}

# 磁盘空间检查
check_disk_space() {
    local required_space_gb="${1:-5}" # 默认需要5GB
    local check_path="${2:-$PROJECT_ROOT}"
    
    # 获取可用空间 (GB)
    local available_space
    if [[ "$OSTYPE" == "darwin"* ]]; then
        available_space=$(df -g "$check_path" | tail -1 | awk '{print $4}')
    else
        available_space=$(df -BG "$check_path" | tail -1 | awk '{print $4}' | tr -d 'G')
    fi
    
    if [[ "$available_space" -lt "$required_space_gb" ]]; then
        handle_error "磁盘空间不足，需要 ${required_space_gb}GB，可用 ${available_space}GB"
    fi
    
    log_success "磁盘空间检查通过: ${available_space}GB 可用"
}

# 进程检查
check_process_running() {
    local process_name="$1"
    
    if pgrep -f "$process_name" > /dev/null; then
        log_success "进程运行检查通过: $process_name"
        return 0
    else
        log_warning "进程未运行: $process_name"
        return 1
    fi
}

# 端口检查
check_port_available() {
    local port="$1"
    
    if command -v lsof &> /dev/null; then
        if lsof -i ":$port" > /dev/null 2>&1; then
            handle_error "端口 $port 已被占用"
        else
            log_success "端口 $port 可用"
        fi
    else
        log_warning "lsof命令不可用，跳过端口检查"
    fi
}

# 时间戳函数
timestamp() {
    date '+%Y-%m-%d %H:%M:%S'
}

# 执行命令并记录时间
timed_execute() {
    local description="$1"
    shift
    
    log_info "开始执行: $description"
    local start_time=$(date +%s)
    
    "$@" || handle_error "执行失败: $description"
    
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    log_success "执行完成: $description (耗时: ${duration}秒)"
}

# 安全地清理临时文件
cleanup_temp_files() {
    local temp_dir="${1:-$PROJECT_ROOT/tmp}"
    
    if [[ -d "$temp_dir" ]]; then
        log_info "清理临时文件: $temp_dir"
        rm -rf "$temp_dir"
        log_success "临时文件清理完成"
    fi
}

# 等待用户确认
confirm_action() {
    local message="${1:-是否继续执行?}"
    local default_yes="${2:-false}"
    
    if [[ "$default_yes" == "true" ]]; then
        local prompt=" [Y/n]"
    else
        local prompt=" [y/N]"
    fi
    
    read -p "$(echo -e "${YELLOW}$message$prompt ${NC}")" -n 1 -r
    echo
    
    if [[ "$default_yes" == "true" ]]; then
        [[ $REPLY =~ ^[Nn]$ ]] && return 1 || return 0
    else
        [[ $REPLY =~ ^[Yy]$ ]] && return 0 || return 1
    fi
}

# 脚本初始化函数
init_script() {
    local script_name="${1:-$(basename "$0")}"
    local required_tools="${2:-}"
    
    log_info "=== $script_name 开始执行 ==="
    log_info "执行时间: $(timestamp)"
    log_info "项目根目录: $PROJECT_ROOT"
    log_info "当前用户: $(whoami)"
    log_info "操作系统: $OSTYPE"
    
    # 基础环境检查
    check_os
    check_git
    
    # 检查必需工具
    if [[ -n "$required_tools" ]]; then
        IFS=',' read -ra TOOLS <<< "$required_tools"
        for tool in "${TOOLS[@]}"; do
            tool=$(echo "$tool" | xargs) # 去除空格
            case "$tool" in
                docker) check_docker ;;
                rust) check_rust ;;
                *) 
                    if ! command -v "$tool" &> /dev/null; then
                        handle_error "必需工具未安装: $tool"
                    else
                        log_success "工具检查通过: $tool"
                    fi
                    ;;
            esac
        done
    fi
    
    log_success "脚本初始化完成"
}

# 脚本结束函数
finish_script() {
    local script_name="${1:-$(basename "$0")}"
    local exit_code="${2:-0}"
    
    if [[ "$exit_code" -eq 0 ]]; then
        log_success "=== $script_name 执行成功完成 ==="
    else
        log_error "=== $script_name 执行失败 ==="
    fi
    
    log_info "结束时间: $(timestamp)"
    exit "$exit_code"
}

# 导出函数供其他脚本使用
export -f log_info log_success log_warning log_error handle_error
export -f check_os check_docker check_git check_rust check_internet
export -f load_config validate_env_vars ensure_directory check_file_exists
export -f check_disk_space check_process_running check_port_available
export -f timestamp timed_execute cleanup_temp_files confirm_action
export -f init_script finish_script

# 设置项目根目录环境变量
export PROJECT_ROOT