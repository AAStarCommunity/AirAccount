#!/bin/bash
# Teaclave TrustZone SDK Development Environment Setup
# Based on: https://teaclave.apache.org/trustzone-sdk-docs/emulate-and-dev-in-docker.md

set -e

DOCKER_IMAGE="teaclave/teaclave-trustzone-emulator-nostd-optee-4.5.0-expand-memory:latest"
CONTAINER_NAME="teaclave_dev_env"
SDK_PATH="$(pwd)/third_party/teaclave-trustzone-sdk"

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== Teaclave TrustZone Dev Environment Setup ===${NC}"

# Step 1: Pull Docker image
step1_pull_image() {
    echo -e "${GREEN}[Step 1/6] Pulling Docker image...${NC}"
    docker pull $DOCKER_IMAGE
    echo -e "${GREEN}✓ Docker image pulled${NC}\n"
}

# Step 2: Start development container
step2_start_container() {
    echo -e "${GREEN}[Step 2/6] Starting development container...${NC}"

    # Stop existing container if running
    if docker ps -a | grep -q $CONTAINER_NAME; then
        echo "Stopping existing container..."
        docker stop $CONTAINER_NAME 2>/dev/null || true
        docker rm $CONTAINER_NAME 2>/dev/null || true
    fi

    docker run -it -d \
        --name $CONTAINER_NAME \
        -v "$SDK_PATH":/root/teaclave_sdk_src \
        -w /root/teaclave_sdk_src \
        $DOCKER_IMAGE

    echo -e "${GREEN}✓ Container started: $CONTAINER_NAME${NC}\n"
}

# Step 3: Build Hello World example
step3_build_example() {
    echo -e "${GREEN}[Step 3/6] Building Hello World example...${NC}"
    docker exec $CONTAINER_NAME bash -l -c \
        "cd /root/teaclave_sdk_src && make -C examples/hello_world-rs/"
    echo -e "${GREEN}✓ Build completed${NC}\n"
}

# Step 4: Sync artifacts to emulator
step4_sync_artifacts() {
    echo -e "${GREEN}[Step 4/6] Syncing artifacts to emulator...${NC}"
    docker exec $CONTAINER_NAME bash -l -c \
        "cd /root/teaclave_sdk_src && make -C examples/hello_world-rs/ emulate"
    echo -e "${GREEN}✓ Artifacts synced${NC}\n"
}

# Step 5-6: Interactive mode instructions
show_interactive_instructions() {
    echo -e "${YELLOW}=== Interactive Mode Required ===${NC}"
    echo -e "The remaining steps require multiple terminals:\n"

    echo -e "${GREEN}Terminal 1 (QEMU Control):${NC}"
    echo "  docker exec -it $CONTAINER_NAME bash -l -c \"LISTEN_MODE=ON start_qemuv8\""
    echo ""

    echo -e "${GREEN}Terminal 2 (Guest VM Shell):${NC}"
    echo "  docker exec -it $CONTAINER_NAME bash -l -c \"listen_on_guest_vm_shell\""
    echo ""

    echo -e "${GREEN}Terminal 3 (Secure World Log):${NC}"
    echo "  docker exec -it $CONTAINER_NAME bash -l -c \"listen_on_secure_world_log\""
    echo ""

    echo -e "${GREEN}Then in Terminal 2 (Guest VM), run:${NC}"
    echo "  ./host/hello_world-rs"
    echo ""
}

# Main execution
case "${1:-all}" in
    "1"|"pull")
        step1_pull_image
        ;;
    "2"|"start")
        step2_start_container
        ;;
    "3"|"build")
        step3_build_example
        ;;
    "4"|"sync")
        step4_sync_artifacts
        ;;
    "interactive"|"run")
        show_interactive_instructions
        ;;
    "clean")
        echo "Stopping and removing container..."
        docker stop $CONTAINER_NAME 2>/dev/null || true
        docker rm $CONTAINER_NAME 2>/dev/null || true
        echo "✓ Cleaned up"
        ;;
    "all")
        step1_pull_image
        step2_start_container
        step3_build_example
        step4_sync_artifacts
        show_interactive_instructions
        ;;
    *)
        echo "Usage: $0 {all|1|2|3|4|interactive|clean}"
        echo ""
        echo "Commands:"
        echo "  all          - Run steps 1-4 automatically"
        echo "  1|pull       - Pull Docker image"
        echo "  2|start      - Start container"
        echo "  3|build      - Build Hello World example"
        echo "  4|sync       - Sync artifacts"
        echo "  interactive  - Show interactive mode instructions"
        echo "  clean        - Stop and remove container"
        exit 1
        ;;
esac