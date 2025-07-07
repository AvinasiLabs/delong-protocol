#!/bin/bash

# DeLong Protocol Environment Setup Script
# This script helps set up the environment for Docker deployment

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DOCKER_DIR="$(dirname "$SCRIPT_DIR")"
ENV_FILE="$DOCKER_DIR/.env"
TEMPLATE_FILE="$DOCKER_DIR/env.template"

echo "🚀 DeLong Protocol Environment Setup"
echo "=================================="

# Check if .env already exists
if [ -f "$ENV_FILE" ]; then
    echo "⚠️  .env file already exists at $ENV_FILE"
    read -p "Do you want to overwrite it? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "❌ Setup cancelled"
        exit 1
    fi
fi

# Copy template
if [ ! -f "$TEMPLATE_FILE" ]; then
    echo "❌ Template file not found at $TEMPLATE_FILE"
    exit 1
fi

cp "$TEMPLATE_FILE" "$ENV_FILE"
echo "✅ Created .env file from template"

# Generate secure JWT secret
if command -v openssl &> /dev/null; then
    JWT_SECRET=$(openssl rand -hex 32)
    sed -i "s/your_super_secret_jwt_key_change_this_in_production/$JWT_SECRET/" "$ENV_FILE"
    echo "✅ Generated secure JWT secret"
else
    echo "⚠️  OpenSSL not found. Please manually set JWT_SECRET in .env file"
fi

# Check if foundry is installed for key generation
if command -v cast &> /dev/null; then
    echo ""
    echo "🔑 Blockchain Account Setup"
    echo "=========================="
    read -p "Do you want to generate a new Ethereum account for testing? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo "Generating new Ethereum account..."
        ACCOUNT_INFO=$(cast wallet new)
        PRIVATE_KEY=$(echo "$ACCOUNT_INFO" | grep "Private key:" | cut -d' ' -f3)
        ADDRESS=$(echo "$ACCOUNT_INFO" | grep "Address:" | cut -d' ' -f2)
        
        # Update .env with new private key
        sed -i "s/0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80/$PRIVATE_KEY/" "$ENV_FILE"
        
        echo "✅ Generated new account:"
        echo "   Address: $ADDRESS"
        echo "   Private Key: $PRIVATE_KEY"
        echo ""
        echo "⚠️  IMPORTANT: This account needs Sepolia ETH for blockchain operations"
        echo "   Get test ETH from: https://sepoliafaucet.com/"
        echo "   Send to address: $ADDRESS"
    fi
else
    echo "⚠️  Foundry not found. Using default test account."
    echo "   Install Foundry: curl -L https://foundry.paradigm.xyz | bash"
fi

# Database setup
echo ""
echo "💾 Database Configuration"
echo "========================"
read -p "Do you want to use custom PostgreSQL credentials? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "Enter PostgreSQL configuration:"
    read -p "Database name (default: delong): " DB_NAME
    read -p "Database user (default: delong): " DB_USER
    read -s -p "Database password: " DB_PASSWORD
    echo
    
    DB_NAME=${DB_NAME:-delong}
    DB_USER=${DB_USER:-delong}
    
    if [ -n "$DB_PASSWORD" ]; then
        # Update .env with new credentials
        sed -i "s/POSTGRES_USER=delong/POSTGRES_USER=$DB_USER/" "$ENV_FILE"
        sed -i "s/POSTGRES_PASSWORD=delong_test_2025/POSTGRES_PASSWORD=$DB_PASSWORD/" "$ENV_FILE"
        sed -i "s/POSTGRES_DATABASE=delong/POSTGRES_DATABASE=$DB_NAME/" "$ENV_FILE"
        sed -i "s|postgresql://delong:delong_test_2025@postgres:5432/delong|postgresql://$DB_USER:$DB_PASSWORD@postgres:5432/$DB_NAME|" "$ENV_FILE"
        echo "✅ Updated database credentials"
    fi
fi

# Network configuration
echo ""
echo "🌐 Network Configuration"
echo "======================="
echo "Select blockchain network:"
echo "1) Local (Anvil - recommended for development)"
echo "2) Sepolia testnet"
read -p "Choice (1-2): " -n 1 -r NETWORK_CHOICE
echo

case $NETWORK_CHOICE in
    2)
        echo "Setting up Sepolia testnet..."
        sed -i 's|ETH_HTTP_URL=http://anvil:8545|# ETH_HTTP_URL=http://anvil:8545|' "$ENV_FILE"
        sed -i 's|ETH_WS_URL=ws://anvil:8545|# ETH_WS_URL=ws://anvil:8545|' "$ENV_FILE"
        sed -i 's|CHAIN_ID=31337|# CHAIN_ID=31337|' "$ENV_FILE"
        sed -i 's|# ETH_HTTP_URL=https://ethereum-sepolia-rpc.publicnode.com|ETH_HTTP_URL=https://ethereum-sepolia-rpc.publicnode.com|' "$ENV_FILE"
        sed -i 's|# ETH_WS_URL=wss://ethereum-sepolia-rpc.publicnode.com|ETH_WS_URL=wss://ethereum-sepolia-rpc.publicnode.com|' "$ENV_FILE"
        sed -i 's|# CHAIN_ID=11155111|CHAIN_ID=11155111|' "$ENV_FILE"
        echo "✅ Configured for Sepolia testnet"
        ;;
    *)
        echo "✅ Using local Anvil network (default)"
        ;;
esac

# TEE configuration
echo ""
echo "🔒 TEE Configuration"
echo "==================="
echo "Select TEE environment:"
echo "1) Mock (recommended for development)"
echo "2) Phala Network (production)"
read -p "Choice (1-2): " -n 1 -r TEE_CHOICE
echo

case $TEE_CHOICE in
    2)
        echo "Setting up Phala Network..."
        sed -i 's|TEE_CLIENT_TYPE=mock|TEE_CLIENT_TYPE=phala|' "$ENV_FILE"
        sed -i 's|BLOCKCHAIN_CLIENT_TYPE=mock|BLOCKCHAIN_CLIENT_TYPE=ethereum|' "$ENV_FILE"
        echo "✅ Configured for Phala Network"
        echo "⚠️  Note: Phala Network integration requires additional setup"
        ;;
    *)
        echo "✅ Using mock TEE (default)"
        ;;
esac

echo ""
echo "🎉 Setup Complete!"
echo "================="
echo "Next steps:"
echo "1. Review the .env file: $ENV_FILE"
echo "2. Start the services:"
echo "   cd $DOCKER_DIR"
echo "   docker-compose -f docker-compose.local.yml up -d"
echo "3. Check service health:"
echo "   curl http://localhost:8080/health"
echo ""
echo "For more information, see: $DOCKER_DIR/README.md" 