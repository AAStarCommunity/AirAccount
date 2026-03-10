#!/bin/bash

# KMS Docker Runner Script
# Builds and runs KMS OP-TEE development environment

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

IMAGE_NAME="kms-optee"
IMAGE_TAG="latest"
CONTAINER_NAME="kms-optee-dev"

show_help() {
    cat << EOF
KMS OP-TEE Docker Runner

Usage: $0 [command] [options]

Commands:
  build         Build Docker image
  run           Run interactive container
  exec          Execute command in running container
  stop          Stop running container
  clean         Remove container and image
  logs          Show container logs

  # Quick commands
  check         Build and check environment
  test          Build and run full test
  qemu          Build and start QEMU

Options:
  --no-cache    Build without cache
  --detach      Run container in background

Examples:
  $0 build                    # Build the Docker image
  $0 run                      # Run interactive container
  $0 exec build              # Build KMS TA in container
  $0 test                     # Full build and test
  $0 clean                    # Clean up everything
EOF
}

build_image() {
    local no_cache=""
    if [[ "$1" == "--no-cache" ]]; then
        no_cache="--no-cache"
        log_info "Building with --no-cache"
    fi

    log_info "Building KMS OP-TEE Docker image..."
    cd "$PROJECT_DIR"

    # Check if required directories exist
    if [[ ! -d "third_party/incubator-teaclave-trustzone-sdk" ]]; then
        log_error "Teaclave SDK not found. Please run: git submodule update --init --recursive"
        exit 1
    fi

    if [[ ! -d "kms" ]]; then
        log_error "KMS source code not found"
        exit 1
    fi

    docker build $no_cache -f Dockerfile.kms-optee -t "$IMAGE_NAME:$IMAGE_TAG" .

    if [[ $? -eq 0 ]]; then
        log_success "Docker image built successfully: $IMAGE_NAME:$IMAGE_TAG"
    else
        log_error "Failed to build Docker image"
        exit 1
    fi
}

run_container() {
    local detach=""
    local cmd="shell"

    while [[ $# -gt 0 ]]; do
        case $1 in
            --detach|-d)
                detach="-d"
                shift
                ;;
            *)
                cmd="$1"
                shift
                ;;
        esac
    done

    # Stop existing container if running
    if docker ps | grep -q "$CONTAINER_NAME"; then
        log_info "Stopping existing container..."
        docker stop "$CONTAINER_NAME" >/dev/null
    fi

    # Remove existing container if exists
    if docker ps -a | grep -q "$CONTAINER_NAME"; then
        log_info "Removing existing container..."
        docker rm "$CONTAINER_NAME" >/dev/null
    fi

    log_info "Starting KMS OP-TEE container..."

    # Mount source code for development
    docker run $detach -it \
        --name "$CONTAINER_NAME" \
        --privileged \
        -v "$PROJECT_DIR/kms:/opt/kms" \
        -v "$PROJECT_DIR/scripts:/opt/scripts" \
        -p 8080:8080 \
        "$IMAGE_NAME:$IMAGE_TAG" "$cmd"

    if [[ $? -eq 0 && -z "$detach" ]]; then
        log_success "Container started successfully"
    elif [[ $? -eq 0 && -n "$detach" ]]; then
        log_success "Container started in background: $CONTAINER_NAME"
        log_info "Use '$0 logs' to view logs"
        log_info "Use '$0 exec shell' to get interactive shell"
    else
        log_error "Failed to start container"
        exit 1
    fi
}

exec_in_container() {
    local cmd="${1:-shell}"

    if ! docker ps | grep -q "$CONTAINER_NAME"; then
        log_error "Container $CONTAINER_NAME is not running"
        log_info "Start it with: $0 run --detach"
        exit 1
    fi

    log_info "Executing in container: $cmd"
    docker exec -it "$CONTAINER_NAME" /entrypoint.sh "$cmd"
}

stop_container() {
    if docker ps | grep -q "$CONTAINER_NAME"; then
        log_info "Stopping container..."
        docker stop "$CONTAINER_NAME"
        log_success "Container stopped"
    else
        log_info "Container is not running"
    fi
}

clean_up() {
    log_info "Cleaning up KMS OP-TEE environment..."

    # Stop and remove container
    if docker ps | grep -q "$CONTAINER_NAME"; then
        docker stop "$CONTAINER_NAME" >/dev/null
    fi

    if docker ps -a | grep -q "$CONTAINER_NAME"; then
        docker rm "$CONTAINER_NAME" >/dev/null
    fi

    # Remove image
    if docker images | grep -q "$IMAGE_NAME"; then
        docker rmi "$IMAGE_NAME:$IMAGE_TAG" >/dev/null
    fi

    log_success "Cleanup completed"
}

show_logs() {
    if docker ps | grep -q "$CONTAINER_NAME"; then
        docker logs -f "$CONTAINER_NAME"
    elif docker ps -a | grep -q "$CONTAINER_NAME"; then
        docker logs "$CONTAINER_NAME"
    else
        log_error "Container $CONTAINER_NAME not found"
        exit 1
    fi
}

# Quick command shortcuts
quick_check() {
    build_image && run_container check
}

quick_test() {
    build_image && run_container test
}

quick_qemu() {
    build_image && run_container qemu
}

# Main script logic
case "${1:-help}" in
    "build")
        build_image "$2"
        ;;
    "run")
        shift
        run_container "$@"
        ;;
    "exec")
        exec_in_container "$2"
        ;;
    "stop")
        stop_container
        ;;
    "clean")
        clean_up
        ;;
    "logs")
        show_logs
        ;;
    "check")
        quick_check
        ;;
    "test")
        quick_test
        ;;
    "qemu")
        quick_qemu
        ;;
    "help"|"--help"|"-h")
        show_help
        ;;
    *)
        log_error "Unknown command: $1"
        show_help
        exit 1
        ;;
esac