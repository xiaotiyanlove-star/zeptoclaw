#!/bin/bash
# Add a new tenant to ZeptoClaw multi-tenant deployment
#
# Usage:
#   ./scripts/add-tenant.sh <tenant-name> <telegram-bot-token> <anthropic-api-key>
#
# Example:
#   ./scripts/add-tenant.sh shop-ahmad "123456:ABC..." "sk-ant-..."
#
# This creates:
#   tenants/<tenant-name>/config.json
#
# After running, regenerate the compose file:
#   ./scripts/generate-compose.sh > docker-compose.multi-tenant.yml
#   docker compose -f docker-compose.multi-tenant.yml up -d

set -e

TENANT_NAME="$1"
BOT_TOKEN="$2"
API_KEY="$3"

if [ -z "$TENANT_NAME" ] || [ -z "$BOT_TOKEN" ] || [ -z "$API_KEY" ]; then
    echo "Usage: $0 <tenant-name> <telegram-bot-token> <anthropic-api-key>"
    echo ""
    echo "Example:"
    echo "  $0 shop-ahmad '123456:ABC...' 'sk-ant-...'"
    exit 1
fi

# Validate tenant name (alphanumeric + hyphens only)
if ! echo "$TENANT_NAME" | grep -qE '^[a-zA-Z0-9][a-zA-Z0-9-]*$'; then
    echo "Error: Tenant name must be alphanumeric with hyphens (e.g., shop-ahmad)"
    exit 1
fi

TENANT_DIR="tenants/$TENANT_NAME"

if [ -d "$TENANT_DIR" ]; then
    echo "Error: Tenant '$TENANT_NAME' already exists at $TENANT_DIR"
    exit 1
fi

mkdir -p "$TENANT_DIR"

# Use python3 to generate JSON — handles all escaping correctly
# with zero risk of shell expansion corrupting values.
# python3's json.dumps() properly escapes \, ", $, `, etc.
python3 -c '
import json, sys

config = {
    "agents": {
        "defaults": {
            "workspace": "/data/workspace",
            "model": "anthropic/claude-sonnet-4",
            "max_tokens": 8192,
            "temperature": 0.7,
            "max_tool_iterations": 20
        }
    },
    "providers": {
        "anthropic": {
            "api_key": sys.argv[1]
        }
    },
    "channels": {
        "telegram": {
            "enabled": True,
            "token": sys.argv[2]
        }
    },
    "tools": {
        "web": {
            "search": {
                "api_key": "",
                "max_results": 5
            }
        }
    },
    "heartbeat": {
        "enabled": False,
        "interval_secs": 1800
    },
    "skills": {
        "enabled": True
    }
}

with open(sys.argv[3], "w") as f:
    json.dump(config, f, indent=2)
    f.write("\n")
' "$API_KEY" "$BOT_TOKEN" "$TENANT_DIR/config.json"

echo "Created tenant: $TENANT_NAME"
echo "Config: $TENANT_DIR/config.json"
echo ""
echo "Next steps:"
echo "  1. Edit $TENANT_DIR/config.json to add WhatsApp, Google Sheets, etc."
echo "  2. Regenerate compose file:"
echo "     ./scripts/generate-compose.sh > docker-compose.multi-tenant.yml"
echo "  3. Deploy:"
echo "     docker compose -f docker-compose.multi-tenant.yml up -d"
