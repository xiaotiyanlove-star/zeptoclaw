#!/bin/bash
# Backup all tenant data volumes
#
# Usage:
#   ./scripts/backup-tenants.sh [backup-dir]
#
# Default backup dir: /backups/zeptoclaw/YYYY-MM-DD
# Schedule with cron: 0 3 * * * /opt/zeptoclaw/scripts/backup-tenants.sh

set -e

BACKUP_DIR="${1:-/backups/zeptoclaw/$(date +%Y-%m-%d)}"
mkdir -p "$BACKUP_DIR"

echo "Backing up tenant volumes to $BACKUP_DIR"

count=0
for volume in $(docker volume ls -q | grep -E ".*-data$"); do
    # Check if it's a zeptoclaw tenant volume
    container=$(docker ps --filter "volume=$volume" --format "{{.Names}}" | grep "^zc-" | head -1)
    [ -z "$container" ] && continue

    tenant=$(echo "$container" | sed 's/^zc-//')
    echo "  Backing up $tenant..."

    docker run --rm \
        -v "$volume:/data:ro" \
        -v "$BACKUP_DIR:/backup" \
        alpine tar czf "/backup/$tenant.tar.gz" -C /data .

    count=$((count + 1))
done

echo "Backed up $count tenant(s) to $BACKUP_DIR"
