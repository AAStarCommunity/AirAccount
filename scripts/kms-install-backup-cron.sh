#!/bin/bash
# KMS 自动备份 Cron 任务安装脚本
# 每小时自动备份钱包数据

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
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

log_success() {
    echo -e "${GREEN}✅${NC} $1"
}

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}🔒 KMS 钱包自动备份安装程序${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# 获取项目根目录绝对路径
PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BACKUP_SCRIPT="$PROJECT_ROOT/scripts/kms-backup-wallets.sh"

# 检查备份脚本是否存在
if [ ! -f "$BACKUP_SCRIPT" ]; then
    log_error "备份脚本不存在: $BACKUP_SCRIPT"
    exit 1
fi

# 确保脚本有执行权限
chmod +x "$BACKUP_SCRIPT"
log_info "备份脚本路径: $BACKUP_SCRIPT"

# 检查当前 crontab
log_info "检查现有 cron 任务..."
EXISTING_CRON=$(crontab -l 2>/dev/null | grep -F "kms-backup-wallets.sh" || true)

if [ -n "$EXISTING_CRON" ]; then
    log_warn "已存在 KMS 备份 cron 任务:"
    echo "  $EXISTING_CRON"
    echo ""
    read -p "是否要替换现有任务? (yes/no): " CONFIRM

    if [ "$CONFIRM" != "yes" ]; then
        log_info "取消安装"
        exit 0
    fi

    # 移除旧任务
    log_info "移除旧的 cron 任务..."
    crontab -l 2>/dev/null | grep -v "kms-backup-wallets.sh" | crontab -
fi

# 创建新的 cron 任务
# 格式: 分 时 日 月 周 命令
# 每小时的第0分钟执行
CRON_JOB="0 * * * * $BACKUP_SCRIPT >> $PROJECT_ROOT/logs/kms-backup-cron.log 2>&1"

log_info "添加新的 cron 任务..."
echo ""
echo -e "${BLUE}Cron 任务配置:${NC}"
echo "  执行频率: 每小时一次 (每小时的第0分钟)"
echo "  脚本路径: $BACKUP_SCRIPT"
echo "  日志路径: $PROJECT_ROOT/logs/kms-backup-cron.log"
echo ""

# 创建日志目录
mkdir -p "$PROJECT_ROOT/logs"

# 添加 cron 任务
(crontab -l 2>/dev/null; echo "$CRON_JOB") | crontab -

log_success "Cron 任务安装成功!"
echo ""

# 验证安装
log_info "验证 cron 任务..."
echo ""
echo -e "${BLUE}当前所有 cron 任务:${NC}"
crontab -l | grep --color=always "kms-backup-wallets.sh" || crontab -l
echo ""

# 显示下次执行时间
NEXT_RUN=$(date -v+1H "+%Y-%m-%d %H:00:00" 2>/dev/null || date -d "+1 hour" "+%Y-%m-%d %H:00:00" 2>/dev/null || echo "未知")
log_info "预计下次执行: $NEXT_RUN"
echo ""

# 测试立即执行一次 (可选)
if [ "$1" = "--test" ]; then
    log_info "执行测试备份..."
    echo ""
    bash "$BACKUP_SCRIPT"
    echo ""
    log_success "测试备份完成!"
elif [ -t 0 ]; then
    # 仅在交互模式下询问
    echo -e "${YELLOW}是否立即执行一次备份测试? (yes/no):${NC} "
    read -r TEST_RUN
    if [ "$TEST_RUN" = "yes" ]; then
        log_info "执行测试备份..."
        echo ""
        bash "$BACKUP_SCRIPT"
        echo ""
        log_success "测试备份完成!"
    fi
fi

echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}📊 使用说明${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo "✅ Cron 任务已启动，将每小时自动备份一次"
echo ""
echo "📁 备份位置: ~/.kms-backup/"
echo "📝 执行日志: $PROJECT_ROOT/logs/kms-backup-cron.log"
echo ""
echo "🔧 管理命令:"
echo "  查看 cron 任务:  crontab -l | grep kms-backup"
echo "  查看执行日志:    tail -f $PROJECT_ROOT/logs/kms-backup-cron.log"
echo "  手动执行备份:    $BACKUP_SCRIPT"
echo "  移除 cron 任务:  crontab -l | grep -v kms-backup-wallets.sh | crontab -"
echo ""
echo "⚠️  注意事项:"
echo "  - 确保 API Server 在运行 (http://localhost:3000)"
echo "  - 备份文件包含助记词,请妥善保管"
echo "  - 建议定期检查备份文件的完整性"
echo ""
