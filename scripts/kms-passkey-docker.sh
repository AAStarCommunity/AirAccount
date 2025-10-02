#!/bin/bash
# KMS Passkey Docker Management Script
# Manages isolated Docker environment for KMS-feat-passkey branch

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
IMAGE_NAME="kms-passkey-qemu"
CONTAINER_NAME="kms_passkey_dev"
DOCKERFILE="Dockerfile.kms-passkey-qemu"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_step() {
    echo -e "\n${BLUE}==>${NC} $1\n"
}

# Function to build Docker image
build_image() {
    log_step "Building KMS Passkey Docker Image..."

    if [ ! -f "$PROJECT_ROOT/$DOCKERFILE" ]; then
        log_error "Dockerfile not found: $PROJECT_ROOT/$DOCKERFILE"
        exit 1
    fi

    cd "$PROJECT_ROOT"
    docker build -f "$DOCKERFILE" -t "$IMAGE_NAME:latest" .

    log_info "✅ Docker image built: $IMAGE_NAME:latest"
}

# Function to start container
start_container() {
    log_step "Starting KMS Passkey Docker Container..."

    # Check if container already exists
    if docker ps -a --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
        log_warn "Container $CONTAINER_NAME already exists"

        # Check if running
        if docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
            log_info "Container is already running"
            return 0
        else
            log_info "Starting existing container..."
            docker start "$CONTAINER_NAME"
            return 0
        fi
    fi

    # Start new container
    docker run -d \
        --name "$CONTAINER_NAME" \
        -v "$PROJECT_ROOT:/root/kms_passkey_src" \
        -v "$PROJECT_ROOT/third_party/teaclave-trustzone-sdk:/root/teaclave_sdk_src" \
        -w /root/kms_passkey_src \
        -p 54330:54320 \
        -p 54331:54321 \
        -p 3001:3000 \
        "$IMAGE_NAME:latest" \
        sleep infinity

    log_info "✅ Container started: $CONTAINER_NAME"
    log_info "   Port mappings:"
    log_info "     54330 → 54320 (Guest VM Shell)"
    log_info "     54331 → 54321 (Secure World Log)"
    log_info "     3001  → 3000  (KMS API Server)"
}

# Function to stop container
stop_container() {
    log_step "Stopping KMS Passkey Docker Container..."

    if docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
        docker stop "$CONTAINER_NAME"
        log_info "✅ Container stopped: $CONTAINER_NAME"
    else
        log_warn "Container $CONTAINER_NAME is not running"
    fi
}

# Function to remove container
remove_container() {
    log_step "Removing KMS Passkey Docker Container..."

    if docker ps -a --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
        docker rm -f "$CONTAINER_NAME"
        log_info "✅ Container removed: $CONTAINER_NAME"
    else
        log_warn "Container $CONTAINER_NAME does not exist"
    fi
}

# Function to enter container shell
shell() {
    if ! docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
        log_error "Container $CONTAINER_NAME is not running. Start it first with: $0 start"
        exit 1
    fi

    docker exec -it "$CONTAINER_NAME" /bin/bash
}

# Function to show status
status() {
    log_step "KMS Passkey Docker Status"

    echo "Image: $IMAGE_NAME"
    if docker images --format '{{.Repository}}:{{.Tag}}' | grep -q "^${IMAGE_NAME}:latest$"; then
        echo "  ✅ Image exists"
    else
        echo "  ❌ Image not found (run: $0 build)"
    fi

    echo ""
    echo "Container: $CONTAINER_NAME"
    if docker ps -a --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
        if docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
            echo "  ✅ Running"
        else
            echo "  ⏸️  Stopped (run: $0 start)"
        fi
    else
        echo "  ❌ Not created (run: $0 start)"
    fi
}

# Function to show logs
logs() {
    if docker ps -a --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
        docker logs -f "$CONTAINER_NAME"
    else
        log_error "Container $CONTAINER_NAME does not exist"
        exit 1
    fi
}

# Main command dispatcher
case "${1:-}" in
    build)
        build_image
        ;;
    start)
        start_container
        ;;
    stop)
        stop_container
        ;;
    restart)
        stop_container
        start_container
        ;;
    remove|rm)
        remove_container
        ;;
    shell|sh)
        shell
        ;;
    status)
        status
        ;;
    logs)
        logs
        ;;
    *)
        echo "Usage: $0 {build|start|stop|restart|remove|shell|status|logs}"
        echo ""
        echo "Commands:"
        echo "  build    - Build Docker image"
        echo "  start    - Start Docker container"
        echo "  stop     - Stop Docker container"
        echo "  restart  - Restart Docker container"
        echo "  remove   - Remove Docker container"
        echo "  shell    - Enter container shell"
        echo "  status   - Show status"
        echo "  logs     - Show container logs"
        exit 1
        ;;
esac
