#!/bin/bash
# 测试Hello World示例验证TEE环境 (重构版本)
# 使用统一脚本库和配置管理

# 加载通用脚本库
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib/common.sh"

# 脚本初始化
init_script "Hello World TEE测试" "docker"

# 加载配置
load_config

# 验证必需的环境变量
validate_env_vars "SDK_DIR" "DOCKER_IMAGE" "OPTEE_CLIENT_EXPORT" "TA_DEV_KIT_DIR"

main() {
    log_info "=== 测试Hello World TEE示例 ==="
    
    # 检查SDK和示例目录
    check_sdk_and_examples
    
    # 检查Docker环境
    check_docker_environment
    
    # 构建Hello World示例
    build_hello_world_example
    
    # 验证构建产物
    verify_build_artifacts
    
    log_success "TEE开发环境验证完成！"
}

check_sdk_and_examples() {
    log_info "1. 检查SDK和示例目录..."
    
    if [[ ! -d "$SDK_DIR" ]]; then
        handle_error "Teaclave SDK目录不存在: $SDK_DIR"
    fi
    log_success "SDK目录检查通过: $SDK_DIR"
    
    local hello_world_dir="$SDK_DIR/examples/hello_world-rs"
    if [[ ! -d "$hello_world_dir" ]]; then
        handle_error "Hello World示例目录不存在: $hello_world_dir"
    fi
    log_success "Hello World示例目录检查通过: $hello_world_dir"
    
    log_success "SDK和示例目录检查通过"
}

check_docker_environment() {
    log_info "2. 检查Docker环境..."
    
    # Docker基础检查已在init_script中完成
    
    # 检查Docker镜像
    if docker images "$DOCKER_IMAGE" | grep -q teaclave; then
        log_success "Docker镜像已存在: $DOCKER_IMAGE"
    else
        log_info "Docker镜像不存在，正在拉取..."
        timed_execute "拉取Docker镜像" docker pull "$DOCKER_IMAGE"
    fi
    
    log_success "Docker环境检查完成"
}

build_hello_world_example() {
    log_info "3. 构建Hello World示例..."
    
    local docker_cmd=(
        docker run --rm
        -v "$SDK_DIR:$DOCKER_WORK_DIR"
        -w "$DOCKER_WORK_DIR"
        -e "OPTEE_CLIENT_EXPORT=$OPTEE_CLIENT_EXPORT"
        -e "TA_DEV_KIT_DIR=$TA_DEV_KIT_DIR"
        "$DOCKER_IMAGE"
        make -C examples/hello_world-rs/
    )
    
    log_info "执行Docker构建命令..."
    log_info "容器工作目录: $DOCKER_WORK_DIR"
    log_info "OPTEE_CLIENT_EXPORT: $OPTEE_CLIENT_EXPORT"
    log_info "TA_DEV_KIT_DIR: $TA_DEV_KIT_DIR"
    
    timed_execute "Hello World示例构建" "${docker_cmd[@]}"
    
    log_success "Hello World示例构建成功"
}

verify_build_artifacts() {
    log_info "4. 验证构建产物..."
    
    local ta_path="$SDK_DIR/examples/hello_world-rs/ta/target/$BUILD_TARGET/release"
    local host_path="$SDK_DIR/examples/hello_world-rs/host/target/$BUILD_TARGET/release"
    
    # 检查TA构建产物
    log_info "检查TA构建产物..."
    if [[ -d "$ta_path" ]]; then
        log_success "TA构建目录存在: $ta_path"
        
        # 查找.ta文件
        local ta_files
        ta_files=$(find "$ta_path" -name "*.ta" -type f)
        if [[ -n "$ta_files" ]]; then
            local ta_file_count=$(echo "$ta_files" | wc -l)
            local ta_file_sizes=$(echo "$ta_files" | xargs ls -lh | awk '{print $5, $9}')
            log_success "找到 $ta_file_count 个TA文件:"
            echo "$ta_file_sizes" | while read -r size file; do
                log_info "  - $(basename "$file"): $size"
            done
        else
            log_warning "未找到.ta文件，但构建目录存在"
        fi
    else
        log_error "TA构建目录不存在: $ta_path"
        return 1
    fi
    
    # 检查Host App构建产物
    log_info "检查Host App构建产物..."
    if [[ -d "$host_path" ]]; then
        log_success "Host构建目录存在: $host_path"
        
        # 查找hello_world可执行文件
        local host_binary="$host_path/hello_world-rs"
        if [[ -f "$host_binary" ]]; then
            local host_size=$(ls -lh "$host_binary" | awk '{print $5}')
            log_success "Host可执行文件: hello_world-rs ($host_size)"
        else
            log_warning "未找到hello_world-rs可执行文件"
        fi
    else
        log_error "Host构建目录不存在: $host_path"
        return 1
    fi
    
    log_success "构建产物验证完成"
}

# 清理函数
cleanup() {
    log_info "执行清理操作..."
    # 这里可以添加必要的清理代码
    log_success "清理完成"
}

# 设置清理陷阱
trap cleanup EXIT

# 主程序入口
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
    finish_script "Hello World TEE测试" 0
fi